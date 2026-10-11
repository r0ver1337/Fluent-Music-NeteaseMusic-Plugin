use super::{Client, fail};
use fluent_plugin_api::ApiError;
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;

pub(super) struct Reply {
    pub Bytes: Zeroizing<Vec<u8>>,
    pub Cookies: Vec<String>,
    pub SessionId: String,
    pub SessionKey: Zeroizing<String>,
}
impl Client {
    pub(super) fn raw(
        &self,
        origin: &str,
        path: &str,
        form: Vec<(String, String)>,
        headers: BTreeMap<String, String>,
    ) -> Result<Reply, ApiError> {
        if !matches!(
            origin,
            "https://music.163.com"
                | "https://interface.music.163.com"
                | "https://interfacepc.music.163.com"
                | "https://interface3.music.163.com"
        ) || !path.starts_with('/')
            || path.contains(['?', '#'])
        {
            return Err(fail("URL", "网易云协议地址未获允许"));
        }
        let target = format!("{origin}{path}");
        #[cfg(test)]
        let target = self
            .TestOrigin
            .as_ref()
            .map_or(target.clone(), |origin| format!("{origin}{path}"));
        let body = Zeroizing::new(
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(form)
                .finish(),
        );
        let mut request = self
            .Http
            .post(target)
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded;charset=utf-8",
            )
            .header("Accept", "application/json")
            .timeout(self.ensureTime()?)
            .body(body.to_string());
        for (key, value) in headers {
            request = request.header(key, value);
        }
        let response = request
            .send()
            .map_err(|_| fail("NETWORK", "网易云 HTTPS 请求失败"))?;
        if !response.status().is_success() {
            return Err(ApiError::new(
                "HTTP",
                format!("网易云 HTTP 状态 {}", response.status().as_u16()),
            ));
        }
        let cookies = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|_| fail("COOKIE", "网易云凭据响应头无效"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let getHeader = |name| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_owned()
        };
        let sessionId = getHeader("x-encr-ssid");
        let sessionKey = Zeroizing::new(getHeader("x-encr-sskey"));
        if sessionId.len() > 4096 || sessionKey.len() > 64 || cookies.len() > 64 {
            return Err(fail("RESPONSE_LIMIT", "网易云响应头超过限制"));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail("NETWORK", "网易云响应读取失败"))?;
        self.ensureTime()?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(fail("RESPONSE_LIMIT", "网易云响应超过限制"));
        }
        Ok(Reply {
            Bytes: bytes,
            Cookies: cookies,
            SessionId: sessionId,
            SessionKey: sessionKey,
        })
    }
}
