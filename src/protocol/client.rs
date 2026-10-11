use super::{
    Cookies, Device, checkCode, crypto, fail, nowMillis,
    xeapi::{self, PublicKey},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use fluent_plugin_api::ApiError;
use ring::hmac;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Read,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
pub(crate) enum Crypto {
    Eapi,
    Weapi,
    Xeapi,
}
pub(crate) struct Client {
    pub Device: Device,
    pub Deadline: Instant,
    pub(super) Http: crate::http::Client,
    Key: Option<PublicKey>,
    SessionId: String,
    SessionKey: Zeroizing<String>,
    #[cfg(test)]
    pub(super) TestOrigin: Option<String>,
    #[cfg(test)]
    TestWeapiKey: Option<[u8; 16]>,
}
const androidUa: &str = "NeteaseMusic/9.5.61.260802021928(9005061);Dalvik/2.1.0 (Linux; U; Android 12; HBN-AL00 Build/cd737a2.0)";
const pcUa: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Safari/537.36 Chrome/91.0.4472.164 NeteaseMusicDesktop/3.1.29.205117";
impl Client {
    pub fn new(device: Device) -> Result<Self, ApiError> {
        device.validate()?;
        let http = crate::http::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| fail("NETWORK", "网易云 HTTPS 客户端初始化失败"))?;
        Ok(Self {
            Device: device,
            Deadline: Instant::now() + Duration::from_secs(80),
            Http: http,
            Key: None,
            SessionId: String::new(),
            SessionKey: Zeroizing::new(String::new()),
            #[cfg(test)]
            TestOrigin: None,
            #[cfg(test)]
            TestWeapiKey: None,
        })
    }
    pub fn ensureTime(&self) -> Result<Duration, ApiError> {
        let time = self.Deadline.saturating_duration_since(Instant::now());
        if time.is_zero() {
            return Err(fail("TIMEOUT", "网易云请求已超时"));
        }
        Ok(time.min(Duration::from_secs(15)))
    }
    pub fn resetTransport(&mut self) {
        self.SessionId.clear();
        self.SessionKey = Zeroizing::new(String::new());
    }
    fn ensureKey(&mut self) -> Result<(), ApiError> {
        if self.Key.is_some() {
            return Ok(());
        }
        let nonce: String = crypto::random::<16>()?
            .iter()
            .map(|byte| char::from(b'0' + byte % 10))
            .collect();
        let timestamp = nowMillis()?.to_string();
        let data = json!({"appVersion":"9.5.61", "currentKeyVersion":"", "deviceId":self.Device.Id,
            "nonce":nonce, "os":"android", "requestType":"active", "signature":crypto::sign(&timestamp,&nonce),
            "t1":"", "t2":"", "timestamp":timestamp, "uid":""});
        let reply = self.raw(
            "https://interface.music.163.com",
            "/api/bsr/sk/get",
            formPairs(&data)?,
            BTreeMap::from([
                ("User-Agent".into(), androidUa.into()),
                ("Cookie".into(), format!("deviceId={}", self.Device.Id)),
            ]),
        )?;
        let value: Value = serde_json::from_slice(&reply.Bytes)
            .map_err(|_| fail("SCHEMA", "网易云公钥响应无效"))?;
        checkCode(&value, &[200])?;
        let result = &value["data"];
        let signature = result["signature"]
            .as_str()
            .and_then(|value| STANDARD.decode(value).ok())
            .ok_or_else(|| fail("CRYPTO", "网易云公钥签名缺失"))?;
        let timestamp = result["timestamp"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| result["timestamp"].as_u64().map(|n| n.to_string()))
            .ok_or_else(|| fail("CRYPTO", "网易云公钥时间无效"))?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"mUHCwVNWJbunMqAHf5MImuirT6plvs6VSFW62MGHstFQxhBGdEoIhLItH3djc4+FB/OKty3+lL2rGeoFBpVe5g==");
        hmac::verify(&key, format!("{timestamp}{nonce}").as_bytes(), &signature)
            .map_err(|_| fail("CRYPTO", "网易云公钥签名校验失败"))?;
        let encrypted = result["encryptedData"]
            .as_str()
            .and_then(|value| STANDARD.decode(value).ok())
            .ok_or_else(|| fail("CRYPTO", "网易云公钥密文缺失"))?;
        let plain = Zeroizing::new(crypto::ecb(crypto::staticKey, &encrypted, false)?);
        self.Key = Some(PublicKey::parse(
            &serde_json::from_slice(&plain).map_err(|_| fail("CRYPTO", "网易云公钥解码失败"))?,
        )?);
        Ok(())
    }
    pub fn request(
        &mut self,
        uri: &str,
        mut data: Value,
        kind: Crypto,
        cookies: &mut Cookies,
        accepted: &[i64],
    ) -> Result<Value, ApiError> {
        if !matches!(
            uri,
            "/api/register/anonimous"
                | "/api/login/qrcode/unikey"
                | "/api/login/qrcode/client/login"
                | "/api/nuser/account/get"
                | "/api/cloudsearch/pc"
                | "/api/user/playlist"
                | "/api/v6/playlist/detail"
                | "/api/v3/song/detail"
                | "/api/personalized/playlist"
                | "/api/personalized/newsong"
                | "/api/v1/discovery/recommend/resource"
                | "/api/v3/discovery/recommend/songs"
                | "/api/song/enhance/player/url/v1"
                | "/api/song/lyric/v1"
                | "/api/song/lyric"
                | "/api/song/music/detail/get"
        ) {
            return Err(fail("URL", "网易云接口未获允许"));
        }
        cookies.validate()?;
        let mut headers = BTreeMap::new();
        let mut cookie: BTreeMap<String, String> = BTreeMap::from([
            ("deviceId".into(), self.Device.Id.clone()),
            ("os".into(), "pc".into()),
            (
                "osver".into(),
                "Microsoft-Windows-10-Professional-build-19045-64bit".into(),
            ),
            ("appver".into(), "3.1.17.204416".into()),
            ("channel".into(), "netease".into()),
            ("__csrf".into(), cookies.get("__csrf").unwrap_or("").into()),
        ]);
        for (key, value) in cookies.entries() {
            cookie.insert(key.into(), value.into());
        }
        let suffix = uri
            .strip_prefix("/api/")
            .ok_or_else(|| fail("URL", "网易云接口路径无效"))?;
        let (origin, path, form) = match kind {
            Crypto::Eapi => {
                cookie.insert(
                    "requestId".into(),
                    format!(
                        "{}_{:04}",
                        nowMillis()?,
                        u16::from_be_bytes(crypto::random::<2>()?) % 1000
                    ),
                );
                cookie.insert("versioncode".into(), "140".into());
                cookie.insert("mobilename".into(), "".into());
                cookie.insert("resolution".into(), "1920x1080".into());
                cookie.insert("buildver".into(), (nowMillis()? / 1000).to_string());
                data["e_r"] = false.into();
                data["header"] =
                    serde_json::to_value(&cookie).map_err(|_| fail("SCHEMA", "请求头编码失败"))?;
                headers.insert("User-Agent".into(), pcUa.into());
                (
                    "https://interfacepc.music.163.com",
                    format!("/eapi/{suffix}"),
                    vec![("params".into(), crypto::eapi(uri, &data)?)],
                )
            }
            Crypto::Weapi => {
                data["csrf_token"] = cookies.get("__csrf").unwrap_or("").into();
                data["e_r"] = false.into();
                #[cfg(not(test))]
                let (params, key) = crypto::weapi(&data)?;
                #[cfg(test)]
                let (params, key) = if let Some(key) = self.TestWeapiKey {
                    crypto::weapiWithKey(&data, &key)?
                } else {
                    crypto::weapi(&data)?
                };
                headers.insert("User-Agent".into(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0".into());
                headers.insert("Referer".into(), "https://music.163.com".into());
                (
                    "https://music.163.com",
                    format!("/weapi/{suffix}"),
                    vec![("params".into(), params), ("encSecKey".into(), key)],
                )
            }
            Crypto::Xeapi => {
                self.ensureKey()?;
                cookie.insert("os".into(), "android".into());
                cookie.insert("osver".into(), "16".into());
                cookie.insert("appver".into(), "9.5.61".into());
                cookie.insert("sDeviceId".into(), self.Device.Id.clone());
                cookie.insert("buildver".into(), (nowMillis()? / 1000).to_string());
                headers.insert("User-Agent".into(), androidUa.into());
                headers.insert("X-Client-Enc-State".into(), "ENCRYPTED".into());
                headers.insert("x-aeapi".into(), "true".into());
                for name in ["deviceId", "os", "osver", "appver", "sDeviceId", "buildver"] {
                    headers.insert(
                        format!("x-{}", name.to_ascii_lowercase()),
                        cookie[name].clone(),
                    );
                }
                if let Some(value) = cookies.get("MUSIC_U") {
                    headers.insert("x-music-u".into(), value.into());
                }
                (
                    "https://interface3.music.163.com",
                    format!("/xeapi/{suffix}"),
                    xeapi::encrypt(
                        &data,
                        self.Key.as_ref().unwrap(),
                        &self.SessionId,
                        &self.SessionKey,
                    )?,
                )
            }
        };
        let cookieHeader = cookie
            .iter()
            .map(|(key, value)| format!("{}={}", encodeCookie(key), encodeCookie(value)))
            .collect::<Vec<_>>()
            .join("; ");
        headers.insert("Cookie".into(), cookieHeader);
        let reply = self.raw(origin, &path, form, headers)?;
        let value: Value = if matches!(kind, Crypto::Xeapi) {
            let plain = Zeroizing::new(crypto::ecb(crypto::eapiKey, &reply.Bytes, false)?);
            if plain.starts_with(&[0x1f, 0x8b]) {
                let mut decoded = Zeroizing::new(Vec::new());
                flate2::read::GzDecoder::new(plain.as_slice())
                    .take(8 * 1024 * 1024 + 1)
                    .read_to_end(&mut decoded)
                    .map_err(|_| fail("SCHEMA", "网易云压缩响应无效"))?;
                if decoded.len() > 8 * 1024 * 1024 {
                    return Err(fail("RESPONSE_LIMIT", "网易云解压响应超过限制"));
                }
                serde_json::from_slice(&decoded)
            } else {
                serde_json::from_slice(&plain)
            }
        } else {
            serde_json::from_slice(&reply.Bytes)
        }
        .map_err(|_| fail("SCHEMA", "网易云响应不是合法 JSON"))?;
        checkCode(&value, accepted)?;
        let mut updated = cookies.clone();
        updated.apply(&reply.Cookies)?;
        if !reply.SessionId.is_empty() && !reply.SessionKey.is_empty() {
            if !matches!(reply.SessionKey.len(), 16 | 24 | 32) {
                return Err(fail("CRYPTO", "网易云 xeapi 会话密钥无效"));
            }
            self.SessionId = reply.SessionId;
            self.SessionKey = reply.SessionKey;
        }
        *cookies = updated;
        Ok(value)
    }
    pub fn guest(&mut self, cookies: &mut Cookies) -> Result<(), ApiError> {
        if cookies.guest() || cookies.authorized() {
            return Ok(());
        }
        let key = b"3go8&$8*3*3h0k(2)2";
        let xored: Vec<_> = self
            .Device
            .Id
            .bytes()
            .enumerate()
            .map(|(index, byte)| byte ^ key[index % key.len()])
            .collect();
        let encoded = STANDARD.encode(format!(
            "{} {}",
            self.Device.Id,
            STANDARD.encode(crypto::md5(&xored))
        ));
        self.request(
            "/api/register/anonimous",
            json!({"username":encoded}),
            Crypto::Xeapi,
            cookies,
            &[200],
        )?;
        if !cookies.guest() {
            return Err(fail("LOGIN", "网易云没有返回游客凭据"));
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn deterministicWeapi(&mut self) {
        self.TestWeapiKey = Some(*b"abcdefghijklmnop");
    }
}
fn encodeCookie(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
pub(super) fn formPairs(value: &Value) -> Result<Vec<(String, String)>, ApiError> {
    value
        .as_object()
        .ok_or_else(|| fail("PARAMETERS", "网易云请求参数必须为对象"))?
        .iter()
        .filter(|(key, _)| key.as_str() != "e_r")
        .map(|(key, value)| {
            let text = if let Some(text) = value.as_str() {
                text.to_owned()
            } else {
                serde_json::to_string(value).map_err(|_| fail("SCHEMA", "参数编码失败"))?
            };
            Ok((key.clone(), text))
        })
        .collect()
}
pub(super) fn formData(value: &Value) -> Result<String, ApiError> {
    Ok(url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(formPairs(value)?)
        .finish())
}
