use super::{testing::*, *};
use serde_json::json;

#[test]
fn actualLoopbackEapiValidatesMd5HeadersCookiesAndRemoteErrors() {
    let mut count = 0;
    let server = serve(2, move |path, headers, body| {
        assert_eq!(path, "/eapi/cloudsearch/pc");
        let (uri, data) = decodeEapi(body);
        assert_eq!(uri, "/api/cloudsearch/pc");
        assert_eq!(data["s"], "合成搜索");
        assert_eq!(data["header"]["MUSIC_A"], "synthetic-guest");
        assert_eq!(data["header"]["deviceId"].as_str().unwrap().len(), 52);
        assert!(headers["cookie"].contains("MUSIC_A=synthetic-guest"));
        count += 1;
        if count == 1 {
            (
                vec![(
                    "Set-Cookie".into(),
                    "NMTID=synthetic-server-device; Path=/; Secure".into(),
                )],
                serde_json::to_vec(&json!({"code":200,"result":{}})).unwrap(),
            )
        } else {
            json(&json!({"code":403,"message":"never expose this remote payload"}))
        }
    });
    let mut client = Client::new(Device::generate().unwrap()).unwrap();
    client.testOrigin(&server.Origin);
    let mut cookies = Cookies::default();
    cookies.apply(&["MUSIC_A=synthetic-guest".into()]).unwrap();
    client
        .request(
            "/api/cloudsearch/pc",
            json!({"s":"合成搜索"}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )
        .unwrap();
    assert_eq!(cookies.get("NMTID"), Some("synthetic-server-device"));
    let error = client
        .request(
            "/api/cloudsearch/pc",
            json!({"s":"合成搜索"}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )
        .unwrap_err();
    assert_eq!(error.code, "PERMISSION_DENIED");
    assert!(!error.message.contains("payload"));
    server.finish();
}
#[test]
fn cookiesAreTransactionalBoundedAndDoNotImportBrowserAttributes() {
    let mut cookies = Cookies::default();
    cookies
        .apply(&[
            "MUSIC_U=synthetic-token; Path=/; HttpOnly".into(),
            "unrelated=ignored".into(),
        ])
        .unwrap();
    assert!(cookies.authorized());
    assert_eq!(cookies.entries().count(), 1);
    let before = cookies.clone();
    assert!(cookies.apply(&["MUSIC_U=bad\r\nvalue".into()]).is_err());
    assert!(cookies == before);
    cookies
        .apply(&["MUSIC_U=removed; Max-Age=0".into()])
        .unwrap();
    assert!(!cookies.authorized());
}
#[test]
fn unknownEndpointsTimeoutAndMalformedResponsesFailClosed() {
    let mut client = Client::new(Device::generate().unwrap()).unwrap();
    let mut cookies = Cookies::default();
    assert_eq!(
        client
            .request(
                "/api/guessed",
                json!({}),
                Crypto::Eapi,
                &mut cookies,
                &[200]
            )
            .unwrap_err()
            .code,
        "URL"
    );
    client.Deadline = std::time::Instant::now();
    assert_eq!(
        client
            .request(
                "/api/cloudsearch/pc",
                json!({}),
                Crypto::Eapi,
                &mut cookies,
                &[200]
            )
            .unwrap_err()
            .code,
        "TIMEOUT"
    );
    assert!(checkCode(&json!({"fake":"success"}), &[200]).is_err());
}
