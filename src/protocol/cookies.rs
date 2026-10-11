use super::fail;
use fluent_plugin_api::ApiError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::Zeroize;

const allowed: &[&str] = &["MUSIC_U", "MUSIC_A", "__csrf", "NMTID"];
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Cookies(BTreeMap<String, String>);
impl Drop for Cookies {
    fn drop(&mut self) {
        for value in self.0.values_mut() {
            value.zeroize();
        }
    }
}
impl Cookies {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.0.len() > allowed.len()
            || self.0.iter().any(|(key, value)| {
                !allowed.contains(&key.as_str())
                    || value.len() > 16384
                    || value
                        .bytes()
                        .any(|byte| !(0x21..=0x7e).contains(&byte) || matches!(byte, b';' | b','))
            })
        {
            return Err(fail("COOKIE", "网易云返回的凭据格式无效"));
        }
        Ok(())
    }
    pub fn apply(&mut self, headers: &[String]) -> Result<(), ApiError> {
        let mut next = self.clone();
        for header in headers {
            if header.len() > 32768 {
                return Err(fail("COOKIE", "网易云凭据超过限制"));
            }
            let mut parts = header.split(';');
            let Some((key, value)) = parts.next().and_then(|pair| pair.split_once('=')) else {
                continue;
            };
            let key = key.trim();
            if !allowed.contains(&key) {
                continue;
            }
            let value = value.trim();
            if value.is_empty() || parts.any(|part| part.trim().eq_ignore_ascii_case("Max-Age=0")) {
                next.0.remove(key);
            } else {
                next.0.insert(key.into(), value.into());
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn authorized(&self) -> bool {
        self.get("MUSIC_U").is_some()
    }
    pub fn guest(&self) -> bool {
        self.get("MUSIC_A").is_some()
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
    pub fn sameAuthorization(&self, other: &Self) -> bool {
        ["MUSIC_U", "__csrf"]
            .iter()
            .all(|key| self.get(key) == other.get(key))
    }
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
}
