use crate::protocol::{checkCode, fail};
use fluent_plugin_api::{ApiError, MediaSource, QualityLevel, QualityList, TrackQuality};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const levels: &[(&str, &str)] = &[
    ("hires", "Hi-Res"),
    ("lossless", "无损"),
    ("exhigh", "极高"),
    ("higher", "较高"),
    ("standard", "标准"),
];
pub(crate) fn catalogue() -> QualityList {
    QualityList {
        Levels: levels
            .iter()
            .enumerate()
            .map(|(index, (_, name))| QualityLevel {
                Priority: index as u32 + 1,
                Name: (*name).into(),
                Disabled: false,
            })
            .collect(),
        Items: Vec::new(),
    }
}
pub(crate) fn validateQuality(id: &str) -> Result<usize, ApiError> {
    levels
        .iter()
        .position(|(key, _)| *key == id)
        .ok_or_else(|| fail("QUALITY", "网易云音质不受当前播放器支持"))
}
pub(crate) fn publicUrl(url: &str) -> Result<String, ApiError> {
    let parsed = url::Url::parse(url).map_err(|_| fail("MEDIA", "网易云播放地址无效"))?;
    let host = parsed.host_str().unwrap_or("");
    if !matches!(parsed.scheme(), "https" | "http")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.fragment().is_some()
        || !(host == "music.126.net" || host.ends_with(".music.126.net"))
    {
        return Err(fail("MEDIA", "网易云返回了未允许的播放地址域名"));
    }
    Ok(parsed.into())
}
pub(crate) fn playback(
    value: &Value,
    id: &str,
    quality: &str,
) -> Result<(MediaSource, Option<u64>), ApiError> {
    validateQuality(quality)?;
    let data = value["data"]
        .as_array()
        .ok_or_else(|| fail("SCHEMA", "网易云播放响应缺少列表"))?;
    let matches: Vec<_> = data
        .iter()
        .filter(|value| crate::metadata::id(&value["id"]).ok().as_deref() == Some(id))
        .collect();
    if matches.len() != 1 {
        return Err(fail("SCHEMA", "网易云播放响应歌曲标识不匹配"));
    }
    let item = matches[0];
    checkCode(item, &[200])?;
    if item
        .get("freeTrialInfo")
        .is_some_and(|trial| !trial.is_null())
        || item["freeTimeTrialPrivilege"]["resConsumable"].as_bool() == Some(true)
        || item["freeTrialPrivilege"]["resConsumable"].as_bool() == Some(true)
    {
        return Err(fail(
            "TRIAL_ONLY",
            "网易云当前仅授权试听片段，不能作为完整歌曲播放",
        ));
    }
    let url = item["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .ok_or_else(|| fail("PLAYBACK_DENIED", "网易云未授权当前身份播放此歌曲"))?;
    if item["level"].as_str() != Some(quality) {
        return Err(fail(
            "QUALITY_DENIED",
            "网易云未返回所选音质，不将降级地址冒充该音质",
        ));
    }
    if !matches!(item["type"].as_str(), Some("mp3" | "flac" | "m4a" | "aac")) {
        return Err(fail("MEDIA_FORMAT", "网易云音频格式不受当前播放器支持"));
    }
    Ok((
        MediaSource::Http {
            url: publicUrl(url)?,
            headers: BTreeMap::new(),
        },
        item["size"].as_u64().filter(|size| *size > 0),
    ))
}
pub(crate) fn qualityItem(index: usize, result: Result<Option<u64>, ApiError>) -> TrackQuality {
    let (id, name) = levels[index];
    let (playable, size, reason) = match result {
        Ok(size) => (true, size, None),
        Err(error) => (false, None, Some(error.message)),
    };
    TrackQuality {
        Id: id.into(),
        Name: name.into(),
        Priority: index as u32 + 1,
        FileSizeBytes: size,
        Playable: playable,
        UnavailableReason: reason,
    }
}
