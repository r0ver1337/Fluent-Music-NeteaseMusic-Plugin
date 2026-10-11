use super::{testing::*, *};
use crate::protocol::Cookies;
use std::sync::atomic::Ordering;
fn session() -> Session {
    let mut cookies = Cookies::default();
    cookies
        .apply(&["MUSIC_U=synthetic-token; Path=/; Secure".into()])
        .unwrap();
    Session {
        UserId: "123".into(),
        Cookies: cookies,
    }
}
#[test]
fn deviceAndSessionAreEncryptedAndGenerationSurvivesLogout() {
    let (store, data) = memoryStore();
    let device = store.device().unwrap();
    assert!(device == store.device().unwrap());
    let snapshot = store.snapshot(&device).unwrap();
    let revision = store.save(&device, &session(), &snapshot.Revision).unwrap();
    assert!(store.snapshot(&device).unwrap().Session.is_some());
    for bytes in data.Files.lock().unwrap().values() {
        assert!(!bytes.windows(15).any(|window| window == b"synthetic-token"));
        assert!(
            !bytes
                .windows(device.Id.len())
                .any(|window| window == device.Id.as_bytes())
        );
    }
    let other = sharedStore(data);
    other.clear(&device).unwrap();
    assert!(store.snapshot(&device).unwrap().Session.is_none());
    assert_eq!(
        store.check(&device, &revision).unwrap_err().code,
        "SESSION_REVOKED"
    );
    assert_eq!(
        store.save(&device, &session(), &revision).unwrap_err().code,
        "SESSION_REVOKED"
    );
    assert_eq!(
        store
            .save(&device, &session(), &snapshot.Revision)
            .unwrap_err()
            .code,
        "SESSION_REVOKED"
    );
}
#[test]
fn atomicSaveAndLogoutFailuresNeverPretendSuccessOrDestroyPreviousRecord() {
    let (store, data) = memoryStore();
    let device = store.device().unwrap();
    let revision = store.snapshot(&device).unwrap().Revision;
    let revision = store.save(&device, &session(), &revision).unwrap();
    let bytes = data.Files.lock().unwrap()["session.dat"].clone();
    data.FailWrite.store(true, Ordering::SeqCst);
    assert!(store.save(&device, &session(), &revision).is_err());
    assert!(store.clear(&device).is_err());
    assert_eq!(data.Files.lock().unwrap()["session.dat"], bytes);
    data.FailWrite.store(false, Ordering::SeqCst);
    store.clear(&device).unwrap();
    assert!(store.snapshot(&device).unwrap().Session.is_none());
}
#[test]
fn corruptOrCrossDeviceCredentialsFailClosed() {
    let (store, data) = memoryStore();
    let device = store.device().unwrap();
    store.snapshot(&device).unwrap();
    assert!(store.snapshot(&Device::generate().unwrap()).is_err());
    data.Files
        .lock()
        .unwrap()
        .insert("session.dat".into(), b"plaintext-token".to_vec());
    assert_eq!(
        store.snapshot(&device).err().unwrap().code,
        "SESSION_STORAGE"
    );
}
#[test]
fn generatedGuestUsesSeparateEncryptedRecordAndCannotStoreAuthorizedCookies() {
    let (store, data) = memoryStore();
    let device = store.device().unwrap();
    let mut guest = Cookies::default();
    guest
        .apply(&["MUSIC_A=synthetic-guest; Secure".into()])
        .unwrap();
    store.saveGuest(&device, &guest).unwrap();
    assert!(store.guest(&device).unwrap() == guest);
    assert!(store.saveGuest(&device, &session().Cookies).is_err());
    store.clear(&device).unwrap();
    assert!(store.guest(&device).unwrap() == guest);
    let bytes = data.Files.lock().unwrap()["guest.dat"].clone();
    assert!(!bytes.windows(15).any(|window| window == b"synthetic-guest"));
}
#[test]
fn staleExpiryCannotRevokeNewLoginAndServerDeviceCookiesDoNotChangeLoginRevision() {
    let (store, _) = memoryStore();
    let device = store.device().unwrap();
    let original = store.snapshot(&device).unwrap().Revision;
    let revision = store.save(&device, &session(), &original).unwrap();
    assert_eq!(
        store.clearExpected(&device, &original).unwrap_err().code,
        "SESSION_REVOKED"
    );
    let mut updated = session();
    updated
        .Cookies
        .apply(&["NMTID=synthetic-device-cookie".into()])
        .unwrap();
    store.update(&device, &updated, &revision).unwrap();
    assert_eq!(store.snapshot(&device).unwrap().Revision, revision);
    assert_eq!(
        store
            .snapshot(&device)
            .unwrap()
            .Session
            .unwrap()
            .Cookies
            .get("NMTID"),
        Some("synthetic-device-cookie")
    );
}
