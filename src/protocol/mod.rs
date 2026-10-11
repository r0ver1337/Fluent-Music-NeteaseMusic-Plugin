mod client;
mod cookies;
pub(crate) mod crypto;
mod identity;
#[cfg(test)]
mod publicTests;
mod response;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
mod transport;
mod xeapi;
#[cfg(test)]
mod xeapiTests;
pub(crate) use client::{Client, Crypto};
pub(crate) use cookies::Cookies;
pub(crate) use identity::{Device, Session};
pub(crate) use response::{checkCode, fail};

pub(crate) fn nowMillis() -> Result<u64, fluent_plugin_api::ApiError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_millis() as u64)
        .map_err(|_| fail("CLOCK", "系统时间无效"))
}
