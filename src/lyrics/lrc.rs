use super::invalid;
use fluent_plugin_api::{ApiError, LyricLine, LyricTimeline};

pub(super) fn parse(text: &str) -> Result<LyricTimeline, ApiError> {
    if text.len() > 1024 * 1024 {
        return Err(invalid());
    }
    let mut offset = 0;
    let mut lines = Vec::new();
    let mut plain = Vec::new();
    for raw in text.lines() {
        let mut raw = raw.trim_start_matches('\u{feff}').trim();
        if let Some(value) = raw
            .strip_prefix("[offset:")
            .and_then(|value| value.strip_suffix(']'))
        {
            offset = value.trim().parse().map_err(|_| invalid())?;
            continue;
        }
        let mut times = Vec::new();
        while let Some(tag) = raw.strip_prefix('[').and_then(|raw| raw.split_once(']')) {
            if !tag.0.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                raw = "";
                break;
            }
            times.push(timestamp(tag.0)?);
            raw = tag.1;
        }
        if !times.is_empty() {
            for start in times {
                lines.push(LyricLine {
                    StartMs: Some(start),
                    DurationMs: None,
                    Text: raw.into(),
                    Words: Vec::new(),
                });
            }
        } else if !raw.is_empty() && !raw.starts_with('{') {
            plain.push(raw.to_owned());
        }
        if lines.len() > 10000 {
            return Err(invalid());
        }
    }
    if lines.is_empty() {
        lines = plain
            .into_iter()
            .map(|text| LyricLine {
                StartMs: None,
                DurationMs: None,
                Text: text,
                Words: Vec::new(),
            })
            .collect();
    } else {
        lines.sort_by_key(|line| line.StartMs);
        for index in 0..lines.len().saturating_sub(1) {
            lines[index].DurationMs = lines[index + 1]
                .StartMs
                .zip(lines[index].StartMs)
                .and_then(|(end, start)| u64::try_from(end - start).ok());
        }
    }
    Ok(LyricTimeline {
        OffsetMs: offset,
        Lines: lines,
    })
}
fn timestamp(value: &str) -> Result<i64, ApiError> {
    let (minutes, seconds) = value.split_once(':').ok_or_else(invalid)?;
    let minutes: i64 = minutes.parse().map_err(|_| invalid())?;
    let (seconds, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let seconds: i64 = seconds.parse().map_err(|_| invalid())?;
    if minutes < 0
        || !(0..60).contains(&seconds)
        || fraction.len() > 3
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    let fraction: i64 = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>().map_err(|_| invalid())? * 10i64.pow(3 - fraction.len() as u32)
    };
    minutes
        .checked_mul(60000)
        .and_then(|n| n.checked_add(seconds * 1000 + fraction))
        .ok_or_else(invalid)
}
