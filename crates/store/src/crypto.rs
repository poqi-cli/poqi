use std::sync::Mutex;

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm,
};
use anyhow::{Context, Result};

const KEY_BYTES: usize = 32;
const KEYRING_SERVICE: &str = "com.poqi.connection-store";

#[derive(Clone, Copy)]
enum KeySource {
    Native,
    #[cfg(any(test, feature = "test-support"))]
    Fixed([u8; KEY_BYTES]),
    #[cfg(any(test, feature = "test-support"))]
    Unavailable,
    #[cfg(any(test, feature = "test-support"))]
    Missing,
}

pub(crate) struct KeyManager {
    source: KeySource,
    cached: Mutex<Option<(String, [u8; KEY_BYTES])>>,
}

impl std::fmt::Debug for KeyManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeyManager")
            .field("source", &"redacted")
            .finish_non_exhaustive()
    }
}

impl KeyManager {
    pub(crate) fn native() -> Self {
        Self {
            source: KeySource::Native,
            cached: Mutex::new(None),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn fixed(key: [u8; KEY_BYTES]) -> Self {
        Self {
            source: KeySource::Fixed(key),
            cached: Mutex::new(None),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn unavailable() -> Self {
        Self {
            source: KeySource::Unavailable,
            cached: Mutex::new(None),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn missing() -> Self {
        Self {
            source: KeySource::Missing,
            cached: Mutex::new(None),
        }
    }

    pub(crate) fn load(&self, store_id: &str) -> Result<Option<[u8; KEY_BYTES]>> {
        if let Some((cached_id, key)) = &*self.cached.lock().expect("key cache poisoned") {
            if cached_id == store_id {
                return Ok(Some(*key));
            }
        }
        let key = self.load_uncached(store_id)?;
        if let Some(key) = key {
            *self.cached.lock().expect("key cache poisoned") = Some((store_id.to_string(), key));
        }
        Ok(key)
    }

    pub(crate) fn load_or_create(&self, store_id: &str) -> Result<[u8; KEY_BYTES]> {
        if let Some(key) = self.load(store_id)? {
            return Ok(key);
        }
        let key = Aes256Gcm::generate_key(&mut OsRng).into();
        self.store_native(store_id, &key)?;
        let stored = self.load_uncached(store_id)?.context(
            "native secure storage did not return the connection encryption key after creation",
        )?;
        anyhow::ensure!(
            stored == key,
            "native secure storage returned a different connection encryption key after creation"
        );
        *self.cached.lock().expect("key cache poisoned") = Some((store_id.to_string(), stored));
        Ok(stored)
    }

    fn load_uncached(&self, store_id: &str) -> Result<Option<[u8; KEY_BYTES]>> {
        match self.source {
            KeySource::Native => load_native(store_id),
            #[cfg(any(test, feature = "test-support"))]
            KeySource::Fixed(key) => Ok(Some(key)),
            #[cfg(any(test, feature = "test-support"))]
            KeySource::Unavailable => {
                anyhow::bail!("test secure storage is unavailable")
            }
            #[cfg(any(test, feature = "test-support"))]
            KeySource::Missing => Ok(None),
        }
    }

    fn store_native(&self, store_id: &str, key: &[u8; KEY_BYTES]) -> Result<()> {
        match self.source {
            KeySource::Native => native_keyring_operation(|| {
                keyring_entry(store_id)?.set_secret(key).context(
                    "failed to save the connection encryption key in native secure storage",
                )
            }),
            #[cfg(any(test, feature = "test-support"))]
            KeySource::Fixed(_) => Ok(()),
            #[cfg(any(test, feature = "test-support"))]
            KeySource::Unavailable => {
                anyhow::bail!("test secure storage is unavailable")
            }
            #[cfg(any(test, feature = "test-support"))]
            KeySource::Missing => {
                anyhow::bail!("test secure storage unexpectedly attempted key creation")
            }
        }
    }
}

pub(crate) struct EncryptedValue {
    pub(crate) nonce: Vec<u8>,
    pub(crate) ciphertext: Vec<u8>,
}

pub(crate) fn encrypt(
    key: &[u8; KEY_BYTES],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<EncryptedValue> {
    let cipher = Aes256Gcm::new_from_slice(key).expect("AES-256 key has fixed length");
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to encrypt connection data"))?;
    Ok(EncryptedValue {
        nonce: nonce.to_vec(),
        ciphertext,
    })
}

pub(crate) fn decrypt(
    key: &[u8; KEY_BYTES],
    nonce: &[u8],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>> {
    anyhow::ensure!(nonce.len() == 12, "stored encryption nonce is malformed");
    let cipher = Aes256Gcm::new_from_slice(key).expect("AES-256 key has fixed length");
    cipher
        .decrypt(
            nonce.into(),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| anyhow::anyhow!("stored connection data failed authentication"))
}

fn load_native(store_id: &str) -> Result<Option<[u8; KEY_BYTES]>> {
    native_keyring_operation(|| match keyring_entry(store_id)?.get_secret() {
        Ok(secret) => {
            let key: [u8; KEY_BYTES] = secret.try_into().map_err(|secret: Vec<u8>| {
                anyhow::anyhow!(
                    "native secure storage returned a malformed connection encryption key ({} bytes)",
                    secret.len()
                )
            })?;
            Ok(Some(key))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error)
            .context("failed to read the connection encryption key from native secure storage"),
    })
}

fn keyring_entry(store_id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("v1/{store_id}"))
        .context("failed to access native secure storage for connection encryption")
}

#[cfg(target_os = "linux")]
fn native_keyring_operation<T, F>(operation: F) -> Result<T>
where
    T: Send,
    F: FnOnce() -> Result<T> + Send,
{
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("poqi-native-keyring".to_string())
            .spawn_scoped(scope, operation)
            .context("failed to start native secure-storage worker thread")?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("native secure-storage worker thread panicked"))?
    })
}

#[cfg(not(target_os = "linux"))]
fn native_keyring_operation<T, F>(operation: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    operation()
}

#[cfg(all(test, any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) fn delete_native_key_for_test(store_id: &str) -> Result<()> {
    native_keyring_operation(|| {
        keyring_entry(store_id)?
            .delete_credential()
            .context("failed to delete native test key")
    })
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn store_native_key_for_test(store_id: &str, key: &[u8; KEY_BYTES]) -> Result<()> {
    KeyManager::native().store_native(store_id, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamper_and_wrong_aad_fail_authentication() {
        let key = [0x17; 32];
        let encrypted = encrypt(&key, b"postgres://secret", b"profile-a").expect("encrypt");
        assert!(decrypt(&key, &encrypted.nonce, &encrypted.ciphertext, b"profile-b").is_err());

        let mut tampered = encrypted.ciphertext;
        tampered[0] ^= 1;
        assert!(decrypt(&key, &encrypted.nonce, &tampered, b"profile-a").is_err());
    }

    #[test]
    fn cached_key_is_bound_to_store_identity() {
        let manager = KeyManager::fixed([0x21; 32]);
        manager.load("first").expect("load first key");
        manager.load("second").expect("load second key");
        let cached = manager.cached.lock().expect("key cache poisoned");
        assert_eq!(cached.as_ref().map(|(id, _)| id.as_str()), Some("second"));
    }
}
