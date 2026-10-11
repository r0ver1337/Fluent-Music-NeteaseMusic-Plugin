use fluent_plugin_api::ApiError;
use serde_json::Value;

pub(crate) fn fail(code: &str, message: &str) -> ApiError {
    ApiError::new(code, message)
}
pub(crate) fn checkCode(value: &Value, accepted: &[i64]) -> Result<(), ApiError> {
    let code = value["code"]
        .as_i64()
        .or_else(|| value["code"].as_str()?.parse().ok())
        .ok_or_else(|| fail("SCHEMA", "网易云响应缺少状态码"))?;
    if accepted.contains(&code) {
        return Ok(());
    }
    Err(match code {
        301 | 302 => fail("LOGIN_REQUIRED", "网易云授权已失效，请重新扫码"),
        401 | 403 | 512 => fail("PERMISSION_DENIED", "网易云拒绝了当前身份的访问权限"),
        429 => fail("RATE_LIMITED", "网易云请求过于频繁，请稍后重试"),
        400 | 405 | 406 | 415 | 460 | 461 => ApiError::new(
            "VERIFICATION_REQUIRED",
            format!("网易云要求官方客户端安全验证（状态 {code}），请稍后重试"),
        ),
        _ => ApiError::new("REMOTE", format!("网易云接口返回状态 {code}")),
    })
}
