use super::invalid;
use fluent_plugin_api::{ApiError, LyricLine, LyricTimeline, LyricWord};
use serde_json::Value;

pub(super) fn parse(text: &str) -> Result<LyricTimeline, ApiError> {
    if text.len() > 1024 * 1024 {
        return Err(invalid());
    }
    let mut lines = Vec::new();
    for raw in text.lines() {
        let raw = raw.trim_start_matches('\u{feff}').trim();
        if raw.is_empty() {
            continue;
        }
        if raw.starts_with('{') {
            let value: Value = serde_json::from_str(raw).map_err(|_| invalid())?;
            let time = value["t"].as_i64().ok_or_else(invalid)?;
            let mut text = String::new();
            for item in value["c"].as_array().ok_or_else(invalid)? {
                text.push_str(item["tx"].as_str().ok_or_else(invalid)?);
            }
            if !text.is_empty() {
                lines.push(LyricLine {
                    StartMs: Some(time),
                    DurationMs: None,
                    Text: text,
                    Words: Vec::new(),
                });
            }
            continue;
        }
        let (tag, mut tail) = raw
            .strip_prefix('[')
            .and_then(|raw| raw.split_once(']'))
            .ok_or_else(invalid)?;
        let (start, duration) = tag.split_once(',').ok_or_else(invalid)?;
        let start: i64 = start.parse().map_err(|_| invalid())?;
        let duration: u64 = duration.parse().map_err(|_| invalid())?;
        let lineEnd = start
            .checked_add(i64::try_from(duration).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let mut words = Vec::new();
        let mut text = String::new();
        while !tail.is_empty() {
            let (tag, rest) = tail
                .strip_prefix('(')
                .and_then(|tail| tail.split_once(')'))
                .ok_or_else(invalid)?;
            let numbers = tag.split(',').collect::<Vec<_>>();
            if numbers.len() != 3 {
                return Err(invalid());
            }
            let wordStart = numbers[0].parse::<i64>().map_err(|_| invalid())?;
            let wordDuration = numbers[1].parse::<u64>().map_err(|_| invalid())?;
            let wordEnd = wordStart
                .checked_add(i64::try_from(wordDuration).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            if wordStart < start || wordEnd > lineEnd {
                return Err(invalid());
            }
            numbers[2].parse::<i64>().map_err(|_| invalid())?;
            let next = findTag(rest).unwrap_or(rest.len());
            let word = &rest[..next];
            if !word.is_empty() {
                text.push_str(word);
                words.push(LyricWord {
                    StartMs: wordStart,
                    DurationMs: wordDuration,
                    Text: word.into(),
                });
            }
            tail = &rest[next..];
            if words.len() > 65536 {
                return Err(invalid());
            }
        }
        lines.push(LyricLine {
            StartMs: Some(start),
            DurationMs: Some(duration),
            Text: text,
            Words: words,
        });
        if lines.len() > 10000 {
            return Err(invalid());
        }
    }
    lines.sort_by_key(|line| line.StartMs);
    Ok(LyricTimeline {
        OffsetMs: 0,
        Lines: lines,
    })
}
fn findTag(text: &str) -> Option<usize> {
    text.match_indices('(').find_map(|(index, rest)| {
        let _ = rest;
        let tag = text[index + 1..].split_once(')')?.0;
        let fields = tag.split(',').collect::<Vec<_>>();
        (fields.len() == 3 && fields.iter().all(|field| field.parse::<i64>().is_ok()))
            .then_some(index)
    })
}
