use super::{Cookies, crypto, fail};
use fluent_plugin_api::ApiError;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Device {
    pub Id: String,
}
impl Device {
    pub fn generate() -> Result<Self, ApiError> {
        Ok(Self {
            Id: crypto::hex(&crypto::random::<26>()?),
        })
    }
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.Id.len() != 52
            || !self
                .Id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_lowercase())
        {
            return Err(fail("SESSION_STORAGE", "网易云设备记录无效"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Session {
    pub UserId: String,
    pub Cookies: Cookies,
}
impl Session {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.UserId.is_empty()
            || self.UserId.len() > 24
            || !self.UserId.bytes().all(|byte| byte.is_ascii_digit())
            || !self.Cookies.authorized()
        {
            return Err(fail("SESSION_STORAGE", "网易云授权记录无效"));
        }
        self.Cookies.validate()
    }
}
