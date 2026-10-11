use super::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Default)]
pub(crate) struct MemoryData {
    pub Files: Mutex<BTreeMap<String, Vec<u8>>>,
    pub FailWrite: AtomicBool,
    Locked: AtomicBool,
}
struct MemoryBackend(Arc<MemoryData>);
struct Guard(Arc<MemoryData>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.Locked.store(false, Ordering::SeqCst);
    }
}
impl storage::Backend for MemoryBackend {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, ApiError> {
        Ok(self.0.Files.lock().unwrap().get(path).cloned())
    }
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), ApiError> {
        if self.0.FailWrite.load(Ordering::SeqCst) {
            return Err(storageError("合成原子保存失败"));
        }
        self.0
            .Files
            .lock()
            .unwrap()
            .insert(path.into(), bytes.to_vec());
        Ok(())
    }
    fn lock(&self) -> Result<Box<dyn Send>, ApiError> {
        if self.0.Locked.swap(true, Ordering::SeqCst) {
            return Err(storageError("合成锁占用"));
        }
        Ok(Box::new(Guard(self.0.clone())))
    }
}
pub(crate) fn memoryStore() -> (SessionStore, Arc<MemoryData>) {
    let data = Arc::new(MemoryData::default());
    (sharedStore(data.clone()), data)
}
pub(crate) fn sharedStore(data: Arc<MemoryData>) -> SessionStore {
    SessionStore {
        Backend: Box::new(MemoryBackend(data)),
    }
}
