mod protection;
mod storage;
use crate::protocol::{Cookies, Device, Session, fail};
use fluent_plugin_api::ApiError;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

const magic: &[u8] = b"FMNETEASE\x01";
const maxBytes: usize = 128 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceRecord {
    Version: u32,
    Device: Device,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuestRecord {
    Version: u32,
    Device: Device,
    Cookies: Cookies,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Snapshot {
    Version: u32,
    Device: Device,
    pub Revision: String,
    pub Session: Option<Session>,
}
pub(crate) struct SessionStore {
    Backend: Box<dyn storage::Backend>,
}
impl SessionStore {
    pub fn guest(&self, device: &Device) -> Result<Cookies, ApiError> {
        let _guard = self.Backend.lock()?;
        let Some(record) = self.read::<GuestRecord>("guest.dat")? else {
            return Ok(Cookies::default());
        };
        if record.Version != 1
            || record.Device != *device
            || record.Cookies.authorized()
            || !record.Cookies.guest()
        {
            return Err(storageError("网易云游客记录无效"));
        }
        record.Cookies.validate()?;
        Ok(record.Cookies)
    }
    pub fn saveGuest(&self, device: &Device, cookies: &Cookies) -> Result<(), ApiError> {
        cookies.validate()?;
        if cookies.authorized() || !cookies.guest() {
            return Err(storageError("本人凭据不能保存到网易云游客记录"));
        }
        let _guard = self.Backend.lock()?;
        self.write(
            "guest.dat",
            &GuestRecord {
                Version: 1,
                Device: device.clone(),
                Cookies: cookies.clone(),
            },
        )
    }
    pub fn broker() -> Result<Self, ApiError> {
        if !fluent_plugin_abi::io::available() {
            return Err(storageError("网易云凭据仅允许通过宿主 FileApi 保存"));
        }
        Ok(Self {
            Backend: Box::new(storage::Broker),
        })
    }
    pub fn device(&self) -> Result<Device, ApiError> {
        let _guard = self.Backend.lock()?;
        if let Some(record) = self.read::<DeviceRecord>("device.dat")? {
            if record.Version != 1 {
                return Err(storageError("网易云设备记录版本无效"));
            }
            record.Device.validate()?;
            return Ok(record.Device);
        }
        let device = Device::generate()?;
        self.write(
            "device.dat",
            &DeviceRecord {
                Version: 1,
                Device: device.clone(),
            },
        )?;
        Ok(device)
    }
    pub fn snapshot(&self, device: &Device) -> Result<Snapshot, ApiError> {
        let _guard = self.Backend.lock()?;
        if let Some(record) = self.readSnapshot(device)? {
            return Ok(record);
        }
        let record = Snapshot {
            Version: 1,
            Device: device.clone(),
            Revision: uuid::Uuid::new_v4().to_string(),
            Session: None,
        };
        self.write("session.dat", &record)?;
        Ok(record)
    }
    fn readSnapshot(&self, device: &Device) -> Result<Option<Snapshot>, ApiError> {
        let Some(record) = self.read::<Snapshot>("session.dat")? else {
            return Ok(None);
        };
        if record.Version != 1
            || record.Device != *device
            || uuid::Uuid::parse_str(&record.Revision).is_err()
        {
            return Err(storageError("网易云授权记录与设备不匹配或已损坏"));
        }
        if let Some(session) = &record.Session {
            session.validate()?;
        }
        Ok(Some(record))
    }
    pub fn check(&self, device: &Device, revision: &str) -> Result<(), ApiError> {
        let _guard = self.Backend.lock()?;
        let record = self.readSnapshot(device)?.ok_or_else(revoked)?;
        if record.Revision != revision {
            return Err(revoked());
        }
        Ok(())
    }
    pub fn save(
        &self,
        device: &Device,
        session: &Session,
        revision: &str,
    ) -> Result<String, ApiError> {
        session.validate()?;
        let _guard = self.Backend.lock()?;
        let previous = self.readSnapshot(device)?.ok_or_else(revoked)?;
        if previous.Revision != revision {
            return Err(revoked());
        }
        let next = uuid::Uuid::new_v4().to_string();
        self.write(
            "session.dat",
            &Snapshot {
                Version: 1,
                Device: device.clone(),
                Revision: next.clone(),
                Session: Some(session.clone()),
            },
        )?;
        Ok(next)
    }
    pub fn clear(&self, device: &Device) -> Result<String, ApiError> {
        self.clearInner(device, None)
    }
    pub fn clearExpected(&self, device: &Device, revision: &str) -> Result<String, ApiError> {
        self.clearInner(device, Some(revision))
    }
    fn clearInner(&self, device: &Device, expected: Option<&str>) -> Result<String, ApiError> {
        let _guard = self.Backend.lock()?;
        if let Some(expected) = expected {
            let previous = self.readSnapshot(device)?.ok_or_else(revoked)?;
            if previous.Revision != expected {
                return Err(revoked());
            }
        }
        let revision = uuid::Uuid::new_v4().to_string();
        // A durable credential-free revision revokes login challenges in every instance.
        self.write(
            "session.dat",
            &Snapshot {
                Version: 1,
                Device: device.clone(),
                Revision: revision.clone(),
                Session: None,
            },
        )?;
        Ok(revision)
    }
    pub fn update(
        &self,
        device: &Device,
        session: &Session,
        revision: &str,
    ) -> Result<(), ApiError> {
        session.validate()?;
        let _guard = self.Backend.lock()?;
        let previous = self.readSnapshot(device)?.ok_or_else(revoked)?;
        if previous.Revision != revision {
            return Err(revoked());
        }
        let record = Snapshot {
            Version: 1,
            Device: device.clone(),
            Revision: revision.into(),
            Session: Some(session.clone()),
        };
        self.write("session.dat", &record)
    }
    fn read<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, ApiError> {
        let Some(bytes) = self.Backend.read(path)? else {
            return Ok(None);
        };
        if bytes.len() > maxBytes || !bytes.starts_with(magic) {
            return Err(storageError("网易云加密记录格式无效"));
        }
        let plain = Zeroizing::new(protection::unprotect(&bytes[magic.len()..])?);
        if plain.len() > maxBytes {
            return Err(storageError("网易云解密记录超过限制"));
        }
        serde_json::from_slice(&plain)
            .map(Some)
            .map_err(|_| storageError("网易云授权记录内容无效"))
    }
    fn write<T: Serialize>(&self, path: &str, record: &T) -> Result<(), ApiError> {
        let plain = Zeroizing::new(
            serde_json::to_vec(record).map_err(|_| storageError("网易云记录编码失败"))?,
        );
        let sealed = protection::protect(&plain)?;
        if sealed.len() + magic.len() > maxBytes {
            return Err(storageError("网易云加密记录超过限制"));
        }
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(&sealed);
        self.Backend.write(path, &bytes)
    }
}
fn storageError(message: &str) -> ApiError {
    fail("SESSION_STORAGE", message)
}
fn revoked() -> ApiError {
    fail(
        "SESSION_REVOKED",
        "网易云授权已在另一实例注销或更新，请重新扫码",
    )
}
#[cfg(all(test, windows))]
pub(crate) mod testing;
#[cfg(all(test, windows))]
mod tests;
