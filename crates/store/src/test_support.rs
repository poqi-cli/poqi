use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::Store;

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
pub(crate) const TEST_KEY: [u8; 32] = [0x42; 32];

pub(crate) struct TestStore {
    store: Store,
    dir: PathBuf,
}

impl TestStore {
    pub(crate) fn new() -> Self {
        let mut dir = std::env::temp_dir();
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        dir.push(format!("poqi-store-test-{id}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        let store_path = dir.join("store.sqlite");
        Self {
            store: Store::new_with_test_key(Some(store_path), TEST_KEY),
            dir,
        }
    }

    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    pub(crate) fn db_path(&self) -> PathBuf {
        self.dir.join("store.sqlite")
    }
}

impl Drop for TestStore {
    fn drop(&mut self) {
        if let Err(err) = remove_dir(&self.dir) {
            eprintln!("failed to clean up temp store: {err}");
        }
    }
}

fn remove_dir(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
