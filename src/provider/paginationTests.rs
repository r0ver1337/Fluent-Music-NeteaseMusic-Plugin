use super::*;
use crate::protocol::testing::{decodeEapi, decodeWeapi, form, json, serve};

fn song(id: u64) -> Value {
    json!({"id":id,"name":format!("合成歌曲{id}"),"ar":[{"name":"甲"},{"name":"乙"}],"al":{"name":"合成专辑"},"dt":123456})
}
fn provider(origin: &str) -> Provider {
    let (store, _) = crate::sessionStore::testing::memoryStore();
    let mut provider = Provider::withStore(store);
    provider.ensureClient().unwrap();
    provider.client().testOrigin(origin);
    provider
        .Guest
        .apply(&["MUSIC_A=synthetic-guest".into()])
        .unwrap();
    provider.restore().unwrap();
    provider
}
#[test]
fn fullPlaylistUsesTrackIdsThenOrderedSongDetailBatchesAndBoundCursor() {
    let mut call = 0;
    let server = serve(4, move |path, _, body| {
        call += 1;
        if call == 1 {
            assert_eq!(path, "/eapi/v6/playlist/detail");
            let (_, request) = decodeEapi(body);
            assert_eq!(request["n"], 100000);
            let ids = (1..=123).map(|id| json!({"id":id})).collect::<Vec<_>>();
            return json(
                &json!({"code":200,"playlist":{"id":777,"trackCount":123,"tracks":[song(1)],"trackIds":ids}}),
            );
        }
        assert_eq!(path, "/weapi/v3/song/detail");
        let fields = form(body);
        assert_eq!(fields["encSecKey"].len(), 256);
        let range = match call {
            2 => 1..=50,
            3 => 51..=100,
            _ => 101..=123,
        };
        let request = decodeWeapi(body);
        let ids: Value = serde_json::from_str(request["c"].as_str().unwrap()).unwrap();
        assert_eq!(ids[0]["id"], *range.start());
        assert_eq!(ids.as_array().unwrap().len(), range.clone().count());
        let songs = range.rev().map(song).collect::<Vec<_>>();
        json(&json!({"code":200,"songs":songs}))
    });
    let mut provider = provider(&server.Origin);
    let mut ids = Vec::new();
    let mut cursor = None;
    for page in 0..3 {
        let tracks = provider.playlist("777", page, cursor.as_deref()).unwrap();
        assert_eq!(tracks.page, page);
        assert_eq!(tracks.has_more, page < 2);
        ids.extend(
            tracks
                .items
                .iter()
                .map(|track| track.id.parse::<u64>().unwrap()),
        );
        cursor = tracks.cursor;
    }
    assert_eq!(ids, (1..=123).collect::<Vec<_>>());
    assert_eq!(
        provider
            .playlist("777", 2, Some("bad:100"))
            .unwrap_err()
            .code,
        "CURSOR_EXPIRED"
    );
    server.finish();
}
#[test]
fn missingTrackMetadataAndPartialSnapshotNeverBecomeSuccessfulEmptyPages() {
    let server = serve(2, |path, _, _| {
        if path == "/eapi/v6/playlist/detail" {
            json(
                &json!({"code":200,"playlist":{"id":777,"trackCount":2,"trackIds":[{"id":1},{"id":2}]}}),
            )
        } else {
            json(&json!({"code":200,"songs":[song(1)]}))
        }
    });
    let mut provider = provider(&server.Origin);
    assert_eq!(
        provider.playlist("777", 0, None).unwrap_err().code,
        "PLAYLIST_INCOMPLETE"
    );
    server.finish();
    let server = serve(1, |_, _, _| {
        json(&json!({"code":200,"playlist":{"id":777,"trackCount":500,"trackIds":[{"id":1}]}}))
    });
    let mut provider = self::provider(&server.Origin);
    assert_eq!(
        provider.playlist("777", 0, None).unwrap_err().code,
        "PLAYLIST_INCOMPLETE"
    );
    server.finish();
}
#[test]
fn accountPlaylistDirectoryFetchesEveryPageWithoutInventedPrivateEndpoints() {
    let mut call = 0;
    let server = serve(3, move |path, _, body| {
        assert_eq!(path, "/weapi/user/playlist");
        call += 1;
        let request = decodeWeapi(body);
        assert_eq!(request["uid"], "123");
        assert_eq!(request["limit"], 100);
        assert_eq!(request["offset"], (call - 1) * 100);
        let range = match call {
            1 => 1..=100,
            2 => 101..=200,
            _ => 201..=205,
        };
        let list = range
            .map(|id| json!({"id":id,"name":format!("合成歌单{id}"),"trackCount":10}))
            .collect::<Vec<_>>();
        json(&json!({"code":200,"more":call < 3,"playlist":list}))
    });
    let mut provider = provider(&server.Origin);
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
    provider.Revision = Some(revision);
    provider.Session = Some(session);
    let playlists = provider.library().unwrap();
    assert_eq!(playlists.len(), 205);
    assert_eq!(playlists[204].id, "205");
    server.finish();
}
