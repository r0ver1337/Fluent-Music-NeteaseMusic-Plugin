use crate::protocol::fail;
use fluent_plugin_api::{Account, ApiError, Playlist, Track};
use serde_json::Value;

pub(crate) fn id(value: &Value) -> Result<String, ApiError> {
    let id = value
        .as_u64()
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
        .or_else(|| value.as_str().map(str::to_owned))
        .ok_or_else(|| fail("SCHEMA", "网易云资源标识缺失"))?;
    validateId(&id)?;
    Ok(id)
}
pub(crate) fn validateId(id: &str) -> Result<(), ApiError> {
    if id.is_empty()
        || id.len() > 20
        || !id.bytes().all(|byte| byte.is_ascii_digit())
        || id.parse::<u64>().ok().is_none_or(|value| value == 0)
    {
        return Err(fail("PARAMETERS", "网易云资源标识无效"));
    }
    Ok(())
}
pub(crate) fn text(value: &Value) -> Result<String, ApiError> {
    let value = value
        .as_str()
        .ok_or_else(|| fail("SCHEMA", "网易云文本字段缺失"))?;
    if value.trim().is_empty()
        || value.len() > 8192
        || value
            .chars()
            .any(|ch| ch.is_control() && !ch.is_whitespace())
    {
        return Err(fail("SCHEMA", "网易云文本字段无效"));
    }
    Ok(value.trim().to_owned())
}
pub(crate) fn artwork(value: &Value) -> Option<String> {
    let mut parsed = url::Url::parse(value.as_str()?).ok()?;
    let host = parsed.host_str()?;
    if !matches!(parsed.scheme(), "https" | "http")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || !(host == "music.126.net" || host.ends_with(".music.126.net"))
    {
        return None;
    }
    parsed.set_scheme("https").ok()?;
    Some(parsed.into())
}
pub(crate) fn track(value: &Value) -> Result<Track, ApiError> {
    let song = value.get("song").unwrap_or(value);
    let artists = song
        .get("ar")
        .or_else(|| song.get("artists"))
        .and_then(Value::as_array)
        .ok_or_else(|| fail("SCHEMA", "网易云歌曲缺少歌手列表"))?;
    if artists.len() > 128 {
        return Err(fail("RESPONSE_LIMIT", "网易云歌曲歌手数量超过限制"));
    }
    let artists = artists
        .iter()
        .map(|artist| text(&artist["name"]))
        .collect::<Result<Vec<_>, _>>()?;
    let album = song
        .get("al")
        .or_else(|| song.get("album"))
        .ok_or_else(|| fail("SCHEMA", "网易云歌曲缺少专辑字段"))?;
    let duration = song
        .get("dt")
        .or_else(|| song.get("duration"))
        .and_then(Value::as_u64)
        .ok_or_else(|| fail("SCHEMA", "网易云歌曲缺少毫秒时长"))?;
    if duration > 7 * 24 * 60 * 60 * 1000 {
        return Err(fail("SCHEMA", "网易云歌曲时长无效"));
    }
    let cover = artwork(&album["picUrl"]).or_else(|| artwork(&value["picUrl"]));
    Ok(Track {
        id: id(&song["id"])?,
        title: text(&song["name"])?,
        artists,
        album: album["name"].as_str().unwrap_or("").to_owned(),
        duration_ms: Some(duration),
        provider_id: "netease".into(),
        mid: None,
        cover_url: cover.clone(),
        thumbnail_url: cover,
    })
}
pub(crate) fn tracks(value: &Value) -> Result<Vec<Track>, ApiError> {
    let array = value
        .as_array()
        .ok_or_else(|| fail("SCHEMA", "网易云歌曲列表缺失"))?;
    if array.len() > 1000 {
        return Err(fail("RESPONSE_LIMIT", "网易云歌曲列表超过限制"));
    }
    array.iter().map(track).collect()
}
pub(crate) fn playlist(value: &Value) -> Result<Playlist, ApiError> {
    Ok(Playlist {
        id: id(&value["id"])?,
        title: text(&value["name"])?,
        track_count: value["trackCount"].as_u64(),
        cover_url: artwork(&value["coverImgUrl"]).or_else(|| artwork(&value["picUrl"])),
        subtitle: value["description"]
            .as_str()
            .filter(|text| text.len() <= 512 && !text.trim().is_empty())
            .map(str::to_owned),
    })
}
pub(crate) fn playlists(value: &Value) -> Result<Vec<Playlist>, ApiError> {
    value
        .as_array()
        .ok_or_else(|| fail("SCHEMA", "网易云歌单列表缺失"))?
        .iter()
        .map(playlist)
        .collect()
}
pub(crate) fn account(value: &Value) -> Result<(String, Account), ApiError> {
    let profile = &value["profile"];
    Ok((
        id(&profile["userId"])?,
        Account {
            display_name: text(&profile["nickname"])?,
            avatar_url: artwork(&profile["avatarUrl"]),
        },
    ))
}
pub(crate) fn validateTrack(track: &Track) -> Result<(), ApiError> {
    if track.provider_id != "netease" {
        return Err(fail("PARAMETERS", "歌曲不属于网易云音乐"));
    }
    validateId(&track.id)
}
