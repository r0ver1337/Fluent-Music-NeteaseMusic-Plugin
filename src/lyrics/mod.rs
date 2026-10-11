mod lrc;
mod yrc;
use crate::protocol::fail;
use fluent_plugin_api::{ApiError, LyricTimeline, Lyrics};
use serde_json::Value;

pub(crate) fn parse(value: &Value) -> Result<Lyrics, ApiError> {
    let original = timeline(value, "yrc", "lrc")?;
    let translation = timeline(value, "ytlrc", "tlyric")?;
    if original.is_none()
        && translation.is_none()
        && value["nolyric"].as_bool() != Some(true)
        && value["uncollected"].as_bool() != Some(true)
    {
        return Err(fail("LYRICS_UNAVAILABLE", "网易云未提供此歌曲的歌词"));
    }
    let lyrics = Lyrics {
        Original: original,
        Translation: translation,
        StartMs: None,
    };
    lyrics.validate()?;
    Ok(lyrics)
}
fn timeline(
    value: &Value,
    wordKey: &str,
    lineKey: &str,
) -> Result<Option<LyricTimeline>, ApiError> {
    let words = value[wordKey]["lyric"]
        .as_str()
        .filter(|text| !text.trim().is_empty());
    let lines = value[lineKey]["lyric"]
        .as_str()
        .filter(|text| !text.trim().is_empty());
    if let Some(text) = words {
        let result = if text.lines().any(|line| {
            line.starts_with('[') && line.split(']').next().is_some_and(|tag| tag.contains(','))
        }) {
            yrc::parse(text)
        } else {
            lrc::parse(text)
        };
        match result {
            Ok(timeline) => return Ok(Some(timeline)),
            Err(error) if lines.is_none() => return Err(error),
            Err(_) => {}
        }
    }
    lines.map(lrc::parse).transpose()
}
fn invalid() -> ApiError {
    fail("LYRICS_FORMAT", "网易云歌词时间或逐字格式无效")
}
#[cfg(test)]
mod tests;
