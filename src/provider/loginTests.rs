use super::*;
use crate::protocol::testing::{decodeEapi, json, serve};
use std::sync::atomic::Ordering;

fn provider(
    origin: &str,
) -> (
    Provider,
    std::sync::Arc<crate::sessionStore::testing::MemoryData>,
) {
    let (store, data) = crate::sessionStore::testing::memoryStore();
    let mut provider = Provider::withStore(store);
    provider.ensureClient().unwrap();
    provider.client().testOrigin(origin);
    provider
        .Guest
        .apply(&["MUSIC_A=synthetic-guest".into()])
        .unwrap();
    provider.restore().unwrap();
    (provider, data)
}
#[test]
fn explicitQrLoginWaitScanAuthorizePersistsOnlyValidatedAccount() {
    let mut call = 0;
    let server = serve(5, move |path, _, body| {
        call += 1;
        if path.starts_with("/eapi/") {
            let (_, value) = decodeEapi(body);
            assert_eq!(value["type"], 3);
            if call == 1 {
                return json(&json!({"code":200,"unikey":"synthetic-key"}));
            }
            assert_eq!(value["key"], "synthetic-key");
            match call {
                2 => return json(&json!({"code":801})),
                3 => return json(&json!({"code":802})),
                4 => {
                    return (
                        vec![
                            (
                                "Set-Cookie".into(),
                                "MUSIC_U=synthetic-authorized; Path=/; HttpOnly".into(),
                            ),
                            ("Set-Cookie".into(), "__csrf=synthetic-csrf; Path=/".into()),
                        ],
                        serde_json::to_vec(&json!({"code":803})).unwrap(),
                    );
                }
                _ => panic!(),
            }
        }
        assert_eq!(path, "/weapi/nuser/account/get");
        json(&json!({"code":200,"profile":{"userId":123,"nickname":"合成账号"}}))
    });
    let (mut provider, data) = provider(&server.Origin);
    assert!(provider.Pending.is_none());
    let ResultData::LoginChallenge(challenge) = provider.loginBegin().unwrap() else {
        panic!()
    };
    assert!(
        challenge
            .qr_text
            .starts_with("https://music.163.com/login?codekey=")
    );
    for expected in ["waiting", "scanned", "authorized"] {
        provider.Pending.as_mut().unwrap().NextPoll = Instant::now();
        let ResultData::LoginState(state) = provider
            .loginPoll(&challenge.challenge_id, &|| false)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(serde_json::to_value(state).unwrap()["state"], expected);
    }
    assert!(provider.Session.is_some());
    assert!(provider.Pending.is_none());
    let encrypted = &data.Files.lock().unwrap()["session.dat"];
    assert!(
        !encrypted
            .windows(20)
            .any(|bytes| bytes == b"synthetic-authorized")
    );
    server.finish();
}
#[test]
fn accountVerificationOrAtomicSaveFailureDoesNotCommitNewSession() {
    let mut call = 0;
    let server = serve(3, move |path, _, _| {
        call += 1;
        match call {
            1 => json(&json!({"code":200,"unikey":"synthetic-key"})),
            2 => (
                vec![(
                    "Set-Cookie".into(),
                    "MUSIC_U=synthetic-authorized; Path=/".into(),
                )],
                serde_json::to_vec(&json!({"code":803})).unwrap(),
            ),
            _ => {
                assert_eq!(path, "/weapi/nuser/account/get");
                json(&json!({"code":200,"profile":{"userId":123,"nickname":"合成账号"}}))
            }
        }
    });
    let (mut provider, data) = provider(&server.Origin);
    let ResultData::LoginChallenge(challenge) = provider.loginBegin().unwrap() else {
        panic!()
    };
    let before = data.Files.lock().unwrap()["session.dat"].clone();
    data.FailWrite.store(true, Ordering::SeqCst);
    assert_eq!(
        provider
            .loginPoll(&challenge.challenge_id, &|| false)
            .err()
            .unwrap()
            .code,
        "SESSION_STORAGE"
    );
    assert!(provider.Session.is_none());
    assert_eq!(data.Files.lock().unwrap()["session.dat"], before);
    server.finish();
}
#[test]
fn anotherInstanceLogoutRevokesEvenGuestStartedPendingChallengeBeforeAnyPoll() {
    let server = serve(1, |_, _, _| {
        json(&json!({"code":200,"unikey":"synthetic-key"}))
    });
    let (mut provider, data) = provider(&server.Origin);
    let ResultData::LoginChallenge(challenge) = provider.loginBegin().unwrap() else {
        panic!()
    };
    let other = crate::sessionStore::testing::sharedStore(data);
    other
        .clear(&provider.Client.as_ref().unwrap().Device)
        .unwrap();
    assert_eq!(
        provider
            .loginPoll(&challenge.challenge_id, &|| false)
            .err()
            .unwrap()
            .code,
        "SESSION_REVOKED"
    );
    assert!(provider.Session.is_none());
    server.finish();
}
