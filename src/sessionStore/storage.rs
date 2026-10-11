use super::{maxBytes, storageError};
use fluent_plugin_abi::{
    files::{openCreate, openRead, openWrite, rootData},
    io::{self, RemoteFile},
};
use fluent_plugin_api::ApiError;
use std::time::{Duration, Instant};

pub(super) trait Backend {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, ApiError>;
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), ApiError>;
    fn lock(&self) -> Result<Box<dyn Send>, ApiError>;
}
pub(super) struct Broker;
impl Backend for Broker {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, ApiError> {
        match io::readFile(rootData, path, maxBytes as u64) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(storageError("无法读取网易云沙箱加密记录")),
        }
    }
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), ApiError> {
        io::writeAtomic(path, bytes).map_err(|_| storageError("无法原子保存网易云沙箱加密记录"))
    }
    fn lock(&self) -> Result<Box<dyn Send>, ApiError> {
        let directory = io::root(rootData).map_err(|_| storageError("网易云沙箱数据目录不可用"))?;
        let file = RemoteFile::open(directory, "session.lock", openRead | openWrite | openCreate)
            .map_err(|_| storageError("无法打开网易云授权记录锁"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match file.lock() {
                Ok(()) => return Ok(Box::new(file)),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => return Err(storageError("网易云授权记录正被另一实例占用")),
            }
        }
    }
}
