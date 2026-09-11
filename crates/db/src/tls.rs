use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rustls::{
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    ClientConfig, RootCertStore,
};
use tokio_postgres::config::SslMode;
use tokio_postgres_rustls::MakeRustlsConnect;
use url::Url;

use crate::DatabaseError;

#[derive(Debug, Clone, Default)]
pub struct TlsFileOptions {
    pub root_cert: Option<PathBuf>,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
}

#[derive(Clone)]
pub enum TlsChoice {
    NoTls,
    Rustls(MakeRustlsConnect),
}

impl fmt::Debug for TlsChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTls => formatter.write_str("NoTls"),
            Self::Rustls(_) => formatter.write_str("Rustls(..)"),
        }
    }
}

pub fn effective_ssl_mode(
    ssl_mode: SslMode,
    tls_files: &TlsFileOptions,
) -> Result<SslMode, DatabaseError> {
    if tls_files.root_cert.is_none() {
        return Ok(ssl_mode);
    }

    match ssl_mode {
        SslMode::Disable => Err(DatabaseError::TlsRootCertWithDisabledTls),
        _ => Ok(SslMode::Require),
    }
}

pub fn strip_tls_params(
    uri: &str,
) -> Result<(String, TlsFileOptions, Option<SslMode>), DatabaseError> {
    let Ok(url) = Url::parse(uri) else {
        if uri.starts_with("postgres://") || uri.starts_with("postgresql://") {
            return strip_postgres_url_tls_params(uri);
        }
        return Ok((uri.to_string(), TlsFileOptions::default(), None));
    };

    let mut tls = ParsedTlsParameters::default();
    let mut retained: Vec<(String, String)> = Vec::new();

    for (key, value) in url.query_pairs() {
        if !tls.take(key.as_ref(), value.as_ref())? {
            retained.push((key.into_owned(), value.into_owned()));
        }
    }

    let mut sanitized = url;
    sanitized.set_query(None);
    if !retained.is_empty() {
        let mut serializer = sanitized.query_pairs_mut();
        for (k, v) in retained {
            serializer.append_pair(&k, &v);
        }
    }

    Ok((sanitized.to_string(), tls.files, Some(tls.ssl_mode)))
}

fn strip_postgres_url_tls_params(
    uri: &str,
) -> Result<(String, TlsFileOptions, Option<SslMode>), DatabaseError> {
    let Some((base, query)) = uri.split_once('?') else {
        return Ok((
            uri.to_string(),
            TlsFileOptions::default(),
            Some(SslMode::Require),
        ));
    };
    let mut tls = ParsedTlsParameters::default();
    let mut retained = Vec::new();
    for raw_parameter in query.split('&') {
        if raw_parameter.is_empty() {
            retained.push(raw_parameter);
            continue;
        }
        let Some((key, value)) = url::form_urlencoded::parse(raw_parameter.as_bytes()).next()
        else {
            retained.push(raw_parameter);
            continue;
        };
        if !tls.take(key.as_ref(), value.as_ref())? {
            retained.push(raw_parameter);
        }
    }

    let sanitized = if retained.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", retained.join("&"))
    };
    Ok((sanitized, tls.files, Some(tls.ssl_mode)))
}

struct ParsedTlsParameters {
    files: TlsFileOptions,
    ssl_mode: SslMode,
}

impl Default for ParsedTlsParameters {
    fn default() -> Self {
        Self {
            files: TlsFileOptions::default(),
            ssl_mode: SslMode::Require,
        }
    }
}

impl ParsedTlsParameters {
    fn take(&mut self, key: &str, value: &str) -> Result<bool, DatabaseError> {
        match key {
            "sslrootcert" => self.files.root_cert = Some(PathBuf::from(value)),
            "sslcert" => self.files.client_cert = Some(PathBuf::from(value)),
            "sslkey" => self.files.client_key = Some(PathBuf::from(value)),
            "sslmode" => {
                self.ssl_mode = match value {
                    "" | "require" | "verify-full" => SslMode::Require,
                    "prefer" => SslMode::Prefer,
                    "disable" => SslMode::Disable,
                    _ => return Err(DatabaseError::UnsupportedTlsMode),
                };
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

pub fn build_tls_connector(
    ssl_mode: SslMode,
    tls_files: &TlsFileOptions,
) -> Result<TlsChoice, DatabaseError> {
    let ssl_mode = effective_ssl_mode(ssl_mode, tls_files)?;
    if ssl_mode == SslMode::Disable {
        return Ok(TlsChoice::NoTls);
    }

    let client_auth = match (&tls_files.client_cert, &tls_files.client_key) {
        (None, None) => ClientAuth::None,
        (Some(_), None) | (None, Some(_)) => return Err(DatabaseError::TlsClientAuthIncomplete),
        (Some(cert_path), Some(key_path)) => ClientAuth::with_files(cert_path, key_path)?,
    };

    let root_store = load_root_store(tls_files.root_cert.as_deref())?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());

    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|source| DatabaseError::TlsBuild(Box::new(source)))?
        .with_root_certificates(root_store);
    let config = match client_auth {
        ClientAuth::None => builder.with_no_client_auth(),
        ClientAuth::WithCert { certs, key } => builder
            .with_client_auth_cert(certs, key)
            .map_err(|source| DatabaseError::TlsBuild(Box::new(source)))?,
    };

    Ok(TlsChoice::Rustls(MakeRustlsConnect::new(config)))
}

fn load_root_store(custom_root: Option<&Path>) -> Result<RootCertStore, DatabaseError> {
    let mut root_store = RootCertStore::empty();
    if let Some(path) = custom_root {
        for cert in load_certificates(path)? {
            root_store
                .add(cert)
                .map_err(|_| DatabaseError::TlsCertParse {
                    path: path.display().to_string(),
                })?;
        }
        return Ok(root_store);
    }

    let native = rustls_native_certs::load_native_certs();
    let (valid, _) = root_store.add_parsable_certificates(native.certs);
    if valid == 0 {
        let details = native
            .errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(DatabaseError::LoadNativeCerts { details });
    }
    Ok(root_store)
}

enum ClientAuth {
    None,
    WithCert {
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    },
}

impl ClientAuth {
    fn with_files(cert_path: &Path, key_path: &Path) -> Result<Self, DatabaseError> {
        let certs = load_certificates(cert_path)?;
        let key = load_private_key(key_path)?;
        Ok(Self::WithCert { certs, key })
    }
}

fn load_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, DatabaseError> {
    let bytes = fs::read(path).map_err(|source| DatabaseError::TlsRead {
        path: path.display().to_string(),
        source,
    })?;
    let certs = CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DatabaseError::TlsCertParse {
            path: path.display().to_string(),
        })?;
    if certs.is_empty() {
        return Err(DatabaseError::TlsCertParse {
            path: path.display().to_string(),
        });
    }
    Ok(certs)
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, DatabaseError> {
    let bytes = fs::read(path).map_err(|source| DatabaseError::TlsRead {
        path: path.display().to_string(),
        source,
    })?;
    PrivateKeyDer::from_pem_slice(&bytes).map_err(|_| DatabaseError::TlsKeyParse {
        path: path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair};
    use rustls::{
        client::danger::{ServerCertVerified, ServerCertVerifier},
        client::WebPkiServerVerifier,
        pki_types::{PrivatePkcs8KeyDer, ServerName, UnixTime},
        Error as RustlsError, ServerConfig, ServerConnection, StreamOwned,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::Duration,
    };

    const SSL_REQUEST: [u8; 8] = [0, 0, 0, 8, 4, 210, 22, 47];
    static NEXT_CERTIFICATE_FILE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn strip_tls_parameters_are_removed() {
        let uri = "postgres://user:pass@localhost:5432/db?sslmode=require&sslrootcert=/tmp/ca.pem&search_path=foo";
        let (sanitized, files, ssl_mode) = strip_tls_params(uri).expect("parse TLS parameters");

        assert_eq!(
            sanitized,
            "postgres://user:pass@localhost:5432/db?search_path=foo"
        );
        assert_eq!(files.root_cert, Some(PathBuf::from("/tmp/ca.pem")));
        assert!(files.client_cert.is_none());
        assert!(files.client_key.is_none());
        assert_eq!(ssl_mode, Some(SslMode::Require));
    }

    #[test]
    fn strip_tls_parameters_passthrough_for_non_url() {
        let uri = "host=/tmp user=postgres sslmode=disable";
        let (sanitized, files, ssl_mode) = strip_tls_params(uri).expect("parse keyword DSN");
        assert_eq!(sanitized, uri);
        assert!(files.root_cert.is_none());
        assert!(files.client_cert.is_none());
        assert!(files.client_key.is_none());
        assert_eq!(ssl_mode, None);
    }

    #[test]
    fn multihost_postgres_url_preserves_authority_and_extracts_tls_parameters() {
        let uri = "postgresql://user@host1:1234,host2:5432/db?sslmode=verify-full&sslrootcert=C%3A%2Fcerts%2Froot%20ca.pem&application_name=my%20app";
        let (sanitized, files, ssl_mode) = strip_tls_params(uri).expect("parse multihost URL");

        assert_eq!(
            sanitized,
            "postgresql://user@host1:1234,host2:5432/db?application_name=my%20app"
        );
        assert_eq!(files.root_cert, Some(PathBuf::from("C:/certs/root ca.pem")));
        assert_eq!(ssl_mode, Some(SslMode::Require));
    }

    #[test]
    fn url_tls_mode_defaults_are_secure_and_verify_full_is_supported() {
        for uri in [
            "postgres://user@localhost/db",
            "postgres://user@localhost/db?sslmode=",
            "postgres://user@localhost/db?sslmode=require",
            "postgres://user@localhost/db?sslmode=verify-full",
        ] {
            let (sanitized, _, ssl_mode) = strip_tls_params(uri).expect("parse TLS mode");
            assert_eq!(ssl_mode, Some(SslMode::Require), "URI: {uri}");
            assert!(!sanitized.contains("sslmode"), "URI: {uri}");
        }
    }

    #[test]
    fn repeated_url_tls_mode_uses_the_last_value() {
        for (uri, expected) in [
            (
                "postgres://user@localhost/db?sslmode=prefer&sslmode=require",
                SslMode::Require,
            ),
            (
                "postgres://user@localhost/db?sslmode=require&sslmode=prefer",
                SslMode::Prefer,
            ),
        ] {
            let (_, _, ssl_mode) = strip_tls_params(uri).expect("parse repeated TLS mode");
            assert_eq!(ssl_mode, Some(expected), "URI: {uri}");
        }
    }

    #[test]
    fn unsupported_libpq_tls_modes_are_rejected() {
        let result = strip_tls_params("postgres://user@localhost/db?sslmode=verify-ca");
        assert!(matches!(result, Err(DatabaseError::UnsupportedTlsMode)));
    }

    #[test]
    fn disable_ssl_mode_short_circuits_tls_build() {
        let choice =
            build_tls_connector(SslMode::Disable, &TlsFileOptions::default()).expect("build tls");
        assert!(matches!(choice, TlsChoice::NoTls));
    }

    #[test]
    fn custom_root_requires_tls_and_rejects_explicit_disable() {
        let files = TlsFileOptions {
            root_cert: Some(PathBuf::from("ca.pem")),
            ..TlsFileOptions::default()
        };

        assert_eq!(
            effective_ssl_mode(SslMode::Prefer, &files).expect("upgrade prefer"),
            SslMode::Require
        );
        assert_eq!(
            effective_ssl_mode(SslMode::Require, &files).expect("retain require"),
            SslMode::Require
        );
        assert!(matches!(
            effective_ssl_mode(SslMode::Disable, &files),
            Err(DatabaseError::TlsRootCertWithDisabledTls)
        ));
    }

    #[test]
    fn prefer_without_custom_root_retains_plaintext_fallback_semantics() {
        assert_eq!(
            effective_ssl_mode(SslMode::Prefer, &TlsFileOptions::default()).expect("retain prefer"),
            SslMode::Prefer
        );
    }

    #[test]
    fn client_certificate_requires_matching_key() {
        for files in [
            TlsFileOptions {
                client_cert: Some(PathBuf::from("client.pem")),
                ..TlsFileOptions::default()
            },
            TlsFileOptions {
                client_key: Some(PathBuf::from("client.key")),
                ..TlsFileOptions::default()
            },
        ] {
            assert!(matches!(
                build_tls_connector(SslMode::Require, &files),
                Err(DatabaseError::TlsClientAuthIncomplete)
            ));
        }
    }

    #[test]
    fn custom_root_verifies_chain_and_hostname() {
        let trusted_ca = test_ca();
        let unrelated_ca = test_ca();
        let server_cert = test_server_cert("database.internal", &trusted_ca);

        let trusted = test_verifier(trusted_ca.der().clone());
        assert!(verify(&trusted, &server_cert, "database.internal").is_ok());

        let untrusted = test_verifier(unrelated_ca.der().clone());
        assert!(verify(&untrusted, &server_cert, "database.internal").is_err());
        assert!(verify(&trusted, &server_cert, "other.internal").is_err());
    }

    #[tokio::test]
    async fn default_and_required_tls_do_not_send_startup_after_ssl_refusal() {
        for connection in [
            "postgres://user@localhost:{port}/db".to_string(),
            "postgres://user@localhost:{port}/db?sslmode=".to_string(),
            "postgres://user@localhost:{port}/db?sslmode=require".to_string(),
            "postgres://user@localhost:{port}/db?sslmode=verify-full".to_string(),
            "host=localhost port={port} user=user dbname=db".to_string(),
            "host=localhost port={port} user=user dbname=db sslmode=prefer sslmode=require"
                .to_string(),
        ] {
            let (port, probe) = spawn_tls_refusal_probe();
            let uri = connection.replace("{port}", &port.to_string());
            let result = test_connect(&uri).await;
            assert!(result.is_err(), "TLS refusal must fail: {uri}");
            assert_eq!(
                probe.join().expect("TLS refusal probe thread"),
                RefusalOutcome::NoStartup,
                "connection sent a plaintext startup packet: {uri}"
            );
        }
    }

    #[tokio::test]
    async fn explicit_prefer_retains_fallback_and_keyword_modes_are_last_wins() {
        for connection in [
            "postgres://user@localhost:{port}/db?sslmode=prefer".to_string(),
            "host=localhost port={port} user=user dbname=db sslmode=require sslmode=prefer"
                .to_string(),
        ] {
            let (port, probe) = spawn_tls_refusal_probe();
            let uri = connection.replace("{port}", &port.to_string());
            let result = test_connect(&uri).await;
            assert!(result.is_err(), "probe closes before authentication: {uri}");
            assert_eq!(
                probe.join().expect("TLS refusal probe thread"),
                RefusalOutcome::StartupReceived,
                "explicit prefer must retain plaintext fallback: {uri}"
            );
        }
    }

    #[tokio::test]
    async fn connector_accepts_trusted_custom_ca_and_rejects_wrong_or_untrusted_servers() {
        let trusted_ca = test_ca();
        let unrelated_ca = test_ca();
        let trusted_root = TestCertificateFile::new(&trusted_ca.pem());
        let unrelated_root = TestCertificateFile::new(&unrelated_ca.pem());

        for case in [
            TlsServerCase {
                hostname: "localhost",
                root: Some(trusted_root.path()),
                ssl_mode: "require",
                expected: TlsServerOutcome::StartupReceived,
            },
            TlsServerCase {
                hostname: "localhost",
                root: Some(trusted_root.path()),
                ssl_mode: "verify-full",
                expected: TlsServerOutcome::StartupReceived,
            },
            TlsServerCase {
                hostname: "database.internal",
                root: Some(trusted_root.path()),
                ssl_mode: "require",
                expected: TlsServerOutcome::HandshakeRejected,
            },
            TlsServerCase {
                hostname: "localhost",
                root: Some(unrelated_root.path()),
                ssl_mode: "require",
                expected: TlsServerOutcome::HandshakeRejected,
            },
            TlsServerCase {
                hostname: "localhost",
                root: None,
                ssl_mode: "require",
                expected: TlsServerOutcome::HandshakeRejected,
            },
            TlsServerCase {
                hostname: "database.internal",
                root: Some(trusted_root.path()),
                ssl_mode: "prefer",
                expected: TlsServerOutcome::HandshakeRejected,
            },
        ] {
            let (certificate, key) = test_server_identity(case.hostname, &trusted_ca);
            let (port, server) = spawn_tls_server(certificate, key);
            let mut url =
                Url::parse(&format!("postgres://user@localhost:{port}/db")).expect("fixture URL");
            url.query_pairs_mut().append_pair("sslmode", case.ssl_mode);
            if let Some(root) = case.root {
                url.query_pairs_mut()
                    .append_pair("sslrootcert", &root.to_string_lossy());
            }

            let result = test_connect(url.as_str()).await;
            assert!(result.is_err(), "fixture never authenticates the client");
            assert_eq!(
                server.join().expect("TLS server thread"),
                case.expected,
                "unexpected TLS result for certificate hostname {}",
                case.hostname
            );
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RefusalOutcome {
        NoStartup,
        StartupReceived,
    }

    fn spawn_tls_refusal_probe() -> (u16, thread::JoinHandle<RefusalOutcome>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind TLS refusal probe");
        let port = listener.local_addr().expect("probe address").port();
        let probe = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept probe client");
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .expect("set probe timeout");
            let mut request = [0_u8; SSL_REQUEST.len()];
            stream.read_exact(&mut request).expect("read SSL request");
            assert_eq!(request, SSL_REQUEST);
            stream.write_all(b"N").expect("refuse TLS");

            let mut startup = [0_u8; 1];
            match stream.read(&mut startup) {
                Ok(0) => RefusalOutcome::NoStartup,
                Ok(_) => RefusalOutcome::StartupReceived,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    RefusalOutcome::NoStartup
                }
                Err(error) => panic!("read after TLS refusal: {error}"),
            }
        });
        (port, probe)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TlsServerOutcome {
        StartupReceived,
        HandshakeRejected,
    }

    #[derive(Debug, Clone, Copy)]
    struct TlsServerCase<'a> {
        hostname: &'static str,
        root: Option<&'a Path>,
        ssl_mode: &'static str,
        expected: TlsServerOutcome,
    }

    fn spawn_tls_server(
        certificate: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> (u16, thread::JoinHandle<TlsServerOutcome>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind TLS fixture");
        let port = listener.local_addr().expect("TLS fixture address").port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept TLS client");
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .expect("set TLS fixture timeout");
            let mut request = [0_u8; SSL_REQUEST.len()];
            stream.read_exact(&mut request).expect("read SSL request");
            assert_eq!(request, SSL_REQUEST);
            stream.write_all(b"S").expect("accept TLS");

            let config = ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .expect("TLS protocol versions")
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)
            .expect("TLS server identity");
            let connection = ServerConnection::new(Arc::new(config)).expect("TLS server");
            let mut tls = StreamOwned::new(connection, stream);
            let mut startup_length = [0_u8; 4];
            if tls.read_exact(&mut startup_length).is_err() {
                return TlsServerOutcome::HandshakeRejected;
            }
            let length = u32::from_be_bytes(startup_length) as usize;
            assert!(length >= 8, "invalid PostgreSQL startup packet length");
            let mut startup = vec![0_u8; length - startup_length.len()];
            tls.read_exact(&mut startup)
                .expect("read PostgreSQL startup packet");
            TlsServerOutcome::StartupReceived
        });
        (port, server)
    }

    async fn test_connect(uri: &str) -> Result<(), DatabaseError> {
        let mut profile = crate::ConnectionProfile::new("TLS fixture", uri);
        profile.connect_timeout = Some(Duration::from_secs(3));
        crate::Database::new().connect(&profile).await
    }

    fn test_server_identity(
        hostname: &str,
        issuer: &CertifiedIssuer<'static, KeyPair>,
    ) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        let params = CertificateParams::new(vec![hostname.to_string()]).expect("server SAN");
        let key = KeyPair::generate().expect("server key");
        let certificate = params
            .signed_by(&key, issuer)
            .expect("CA-signed server certificate")
            .der()
            .clone();
        let key = PrivatePkcs8KeyDer::from(key.serialize_der()).into();
        (certificate, key)
    }

    struct TestCertificateFile(PathBuf);

    impl TestCertificateFile {
        fn new(contents: &str) -> Self {
            let sequence = NEXT_CERTIFICATE_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "poqi-tls-test-{}-{sequence}.pem",
                std::process::id()
            ));
            fs::write(&path, contents).expect("write test CA");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestCertificateFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn test_ca() -> CertifiedIssuer<'static, KeyPair> {
        let mut params = CertificateParams::new(Vec::<String>::new()).expect("empty SAN list");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        CertifiedIssuer::self_signed(params, KeyPair::generate().expect("CA key"))
            .expect("self-signed CA")
    }

    fn test_server_cert(
        hostname: &str,
        issuer: &CertifiedIssuer<'static, KeyPair>,
    ) -> CertificateDer<'static> {
        let params = CertificateParams::new(vec![hostname.to_string()]).expect("server SAN");
        params
            .signed_by(&KeyPair::generate().expect("server key"), issuer)
            .expect("CA-signed server certificate")
            .der()
            .clone()
    }

    fn test_verifier(root: CertificateDer<'static>) -> Arc<WebPkiServerVerifier> {
        let mut roots = RootCertStore::empty();
        roots.add(root).expect("test root");
        WebPkiServerVerifier::builder_with_provider(
            Arc::new(roots),
            Arc::new(rustls::crypto::ring::default_provider()),
        )
        .build()
        .expect("test verifier")
    }

    fn verify(
        verifier: &WebPkiServerVerifier,
        server_cert: &CertificateDer<'_>,
        hostname: &'static str,
    ) -> Result<ServerCertVerified, RustlsError> {
        verifier.verify_server_cert(
            server_cert,
            &[],
            &ServerName::try_from(hostname).expect("valid test hostname"),
            &[],
            UnixTime::now(),
        )
    }
}
