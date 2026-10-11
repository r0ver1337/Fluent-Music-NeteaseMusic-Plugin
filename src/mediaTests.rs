use crate::media::*;
use serde_json::{Value, json};
fn response() -> Value {
    json!({"data":[{"code":200,"id":123,"url":"https://m7.music.126.net/synthetic.mp3?token=fixture","type":"mp3","level":"standard","size":2000,"freeTrialInfo":null}]})
}
#[test]
fn onlyExactAuthorizedNonTrialOfficialMediaIsPlayable() {
    assert!(playback(&response(), "123", "standard").is_ok());
    assert_eq!(
        playback(&response(), "123", "lossless").err().unwrap().code,
        "QUALITY_DENIED"
    );
    assert_eq!(
        playback(&response(), "321", "standard").err().unwrap().code,
        "SCHEMA"
    );
    for trial in [json!({"start":100,"end":200}), json!("null"), json!(false)] {
        let mut value = response();
        value["data"][0]["freeTrialInfo"] = trial;
        assert_eq!(
            playback(&value, "123", "standard").err().unwrap().code,
            "TRIAL_ONLY"
        );
    }
    let mut value = response();
    value["data"][0]["url"] = Value::Null;
    assert_eq!(
        playback(&value, "123", "standard").err().unwrap().code,
        "PLAYBACK_DENIED"
    );
    value["data"][0]["code"] = 403.into();
    assert_eq!(
        playback(&value, "123", "standard").err().unwrap().code,
        "PERMISSION_DENIED"
    );
}
#[test]
fn mediaDomainRestrictionsRejectLookalikesCredentialsPortsAndFallbackSources() {
    for url in [
        "https://music.126.net.evil.test/a",
        "https://evil.test/music.126.net",
        "https://user@music.126.net/a",
        "https://m7.music.126.net:444/a",
        "file:///a",
        "https://m7.music.126.net/a#x",
        "https://music.163.com/song/123",
        "https://kugou.com/a",
        "http://127.0.0.1/a",
    ] {
        assert!(publicUrl(url).is_err(), "{url}");
    }
    assert!(publicUrl("https://m8.music.126.net/fixture.flac").is_ok());
    assert!(publicUrl("http://m8.music.126.net/fixture.flac").is_ok());
}
#[test]
fn qualityCatalogueKeepsPermissionDisabledWithoutMakingFileExistenceAuthorization() {
    let mut list = catalogue();
    for index in 0..levels.len() {
        list.Items.push(qualityItem(
            index,
            Err(crate::protocol::fail("PLAYBACK_DENIED", "当前身份未授权")),
        ));
    }
    list.validate().unwrap();
    assert!(list.Items.iter().all(|item| !item.Playable));
    assert!(list.candidates(1).is_empty());
    assert!(validateQuality("dolby").is_err());
}

#[test]
fn bundledNetworkManifestAllowsEverySupportedOfficialPlaybackSchemeAndQuality() {
    let manifest: fluent_plugin_host::NativeManifest =
        serde_json::from_str(include_str!("../plugin.json")).unwrap();
    let policy = fluent_plugin_host::NetworkPolicy::new(&manifest.Manifest).unwrap();
    for scheme in ["http", "https"] {
        for host in ["music.126.net", "m7.music.126.net", "m801.music.126.net"] {
            for (quality, _) in levels {
                let mut value = response();
                let url = format!("{scheme}://{host}/synthetic.mp3?token=fixture%2Bsignature");
                value["data"][0]["url"] = json!(url);
                value["data"][0]["level"] = json!(quality);
                let (source, _) = playback(&value, "123", quality).unwrap();
                let fluent_plugin_api::MediaSource::Http { url: returned, .. } = &source else {
                    panic!("expected official HTTP(S) source")
                };
                assert_eq!(returned, &url);
                assert!(policy.check(returned).is_ok(), "{scheme} {host} {quality}");
                assert!(
                    policy
                        .checkResult(&fluent_plugin_api::ResultData::Media(source))
                        .is_ok()
                );
            }
        }
    }
}

#[test]
fn playbackManifestNeverGrantsHttpAccountApisForeignHostsOrCustomPorts() {
    let manifest: fluent_plugin_host::NativeManifest =
        serde_json::from_str(include_str!("../plugin.json")).unwrap();
    let policy = fluent_plugin_host::NetworkPolicy::new(&manifest.Manifest).unwrap();
    for url in [
        "http://music.163.com/api/nuser/account/get",
        "http://interface.music.163.com/api/register/anonimous",
        "http://music.126.net.evil.test/audio.mp3",
        "http://evilmusic.126.net/audio.mp3",
        "http://user:password@m7.music.126.net/audio.mp3",
        "http://m7.music.126.net:8080/audio.mp3",
        "http://127.0.0.1/audio.mp3",
        "https://other.example/audio.mp3",
    ] {
        assert!(policy.check(url).is_err(), "{url}");
    }
}
