use super::*;
use crate::protocol::testing::{decodeEapi, decodeWeapi, json, serve};
fn setup(origin: &str, authorized: bool) -> Provider {
    let (store, _) = crate::sessionStore::testing::memoryStore();
    let mut provider = Provider::withStore(store);
    provider.ensureClient().unwrap();
    provider.restore().unwrap();
    provider.client().testOrigin(origin);
    provider
        .Guest
        .apply(&["MUSIC_A=synthetic-guest".into()])
        .unwrap();
    if authorized {
        let mut cookies = Cookies::default();
        cookies.apply(&["MUSIC_U=synthetic-user".into()]).unwrap();
        let session = Session {
            UserId: "123".into(),
            Cookies: cookies,
        };
        let revision = provider
            .Store
            .save(
                &provider.Client.as_ref().unwrap().Device,
                &session,
                provider.revision().unwrap(),
            )
            .unwrap();
        provider.Session = Some(session);
        provider.Revision = Some(revision);
    }
    provider
}
fn song() -> Value {
    json!({"id":1,"name":"合成歌曲","ar":[{"name":"甲"},{"name":"乙"}],"al":{"name":"合成专辑"},"dt":123456})
}
#[test]
fn publicFeedLabelsBothSourcesAndKeepsEveryArtistAndDuration() {
    let server = serve(2, |path, _, body| {
        let request = decodeWeapi(body);
        assert_eq!(request["limit"], 30);
        if path == "/weapi/personalized/playlist" {
            json(&json!({"code":200,"result":[{"id":777,"name":"合成歌单"}]}))
        } else {
            assert_eq!(path, "/weapi/personalized/newsong");
            json(&json!({"code":200,"result":[{"song":song()}]}))
        }
    });
    let mut provider = setup(&server.Origin, false);
    let feed = provider.feed().unwrap();
    assert!(!feed.personalized);
    assert_eq!(feed.track_source, RecommendationSource::Public);
    assert_eq!(feed.playlist_source, RecommendationSource::Public);
    assert!(feed.notice.unwrap().contains("游客"));
    assert_eq!(feed.tracks[0].artists.len(), 2);
    assert_eq!(feed.tracks[0].duration_ms, Some(123456));
    server.finish();
}
#[test]
fn actualAccountFailuresStayVisibleWhileExplicitPublicFallbackSucceeds() {
    let server = serve(4, |path, _, _| match path {
        "/weapi/v1/discovery/recommend/resource" | "/weapi/v3/discovery/recommend/songs" => {
            json(&json!({"code":403}))
        }
        "/weapi/personalized/playlist" => {
            json(&json!({"code":200,"result":[{"id":777,"name":"合成公开歌单"}]}))
        }
        "/weapi/personalized/newsong" => json(&json!({"code":200,"result":[{"song":song()}]})),
        _ => panic!(),
    });
    let mut provider = setup(&server.Origin, true);
    let feed = provider.feed().unwrap();
    assert!(!feed.personalized);
    assert_eq!(feed.issues.len(), 2);
    assert!(
        feed.issues
            .iter()
            .all(|issue| issue.error.code == "PERMISSION_DENIED")
    );
    assert!(feed.notice.unwrap().contains("回退"));
    server.finish();
}
#[test]
fn searchPagingUsesExactOffsetAndDeniedRecommendationsRemainErrors() {
    let server = serve(1, |path, _, body| {
        assert_eq!(path, "/eapi/cloudsearch/pc");
        let (_, data) = decodeEapi(body);
        assert_eq!(data["offset"], 150);
        assert_eq!(data["type"], 1);
        json(&json!({"code":200,"result":{"songCount":151,"songs":[song()]}}))
    });
    let mut provider = setup(&server.Origin, false);
    let page = provider.search("fixture", 3).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.has_more);
    server.finish();
    let server = serve(2, |_, _, _| json(&json!({"code":403})));
    let mut provider = setup(&server.Origin, false);
    assert_eq!(provider.feed().unwrap_err().code, "PERMISSION_DENIED");
    server.finish();
}
