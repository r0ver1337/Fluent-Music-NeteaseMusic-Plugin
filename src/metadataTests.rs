use crate::metadata::*;
use serde_json::json;
#[test]
fn allArtistsAndExactMillisecondsSurviveBothConfirmedSongSchemas() {
    let song = json!({"id":123,"name":"合成歌曲","ar":[{"name":"歌手甲"},{"name":"歌手乙"},{"name":"歌手丙"}],"al":{"name":"合成专辑","picUrl":"https://p1.music.126.net/fixture.jpg"},"dt":123456});
    let modern = track(&song).unwrap();
    assert_eq!(modern.artists, ["歌手甲", "歌手乙", "歌手丙"]);
    assert_eq!(modern.duration_ms, Some(123456));
    let old = track(&json!({"id":124,"name":"合成旧格式","artists":[{"name":"甲"},{"name":"乙"}],"album":{"name":"旧专辑"},"duration":98765})).unwrap();
    assert_eq!(old.duration_ms, Some(98765));
    assert_eq!(old.artists.len(), 2);
    assert!(track(&json!({"id":1,"name":"incomplete"})).is_err());
}

#[test]
fn officialHttpArtworkIsUpgradedForSongsPlaylistsAndAccounts() {
    let original = "http://p1.music.126.net/fixture.jpg?param=200y200";
    let secure = "https://p1.music.126.net/fixture.jpg?param=200y200";
    assert_eq!(artwork(&json!(original)).as_deref(), Some(secure));
    let song = json!({"id":123,"name":"合成歌曲","ar":[{"name":"歌手"}],"al":{"name":"专辑","picUrl":original},"dt":123456});
    let song = track(&song).unwrap();
    assert_eq!(song.cover_url.as_deref(), Some(secure));
    assert_eq!(song.thumbnail_url.as_deref(), Some(secure));
    assert_eq!(
        playlist(&json!({"id":456,"name":"合成歌单","coverImgUrl":original}))
            .unwrap()
            .cover_url
            .as_deref(),
        Some(secure)
    );
    assert_eq!(
        account(&json!({"profile":{"userId":789,"nickname":"合成账号","avatarUrl":original}}))
            .unwrap()
            .1
            .avatar_url
            .as_deref(),
        Some(secure)
    );
}

#[test]
fn artworkUpgradeNeverAcceptsCredentialsForeignHostsOrNonWebSchemes() {
    for url in [
        "http://music.126.net.evil.test/a.jpg",
        "http://evil.test/music.126.net/a.jpg",
        "http://user:password@p1.music.126.net/a.jpg",
        "http://p1.music.126.net:8080/a.jpg",
        "file:///a.jpg",
        "ftp://p1.music.126.net/a.jpg",
        "http://127.0.0.1/a.jpg",
    ] {
        assert!(artwork(&json!(url)).is_none());
    }
    assert_eq!(
        artwork(&json!("https://p2.music.126.net/a.jpg")).as_deref(),
        Some("https://p2.music.126.net/a.jpg")
    );
    assert!(artwork(&serde_json::Value::Null).is_none());
    let song = json!({"id":123,"name":"没有封面的歌曲","ar":[],"al":{"name":"专辑"},"dt":123456});
    assert!(track(&song).unwrap().cover_url.is_none());
}
