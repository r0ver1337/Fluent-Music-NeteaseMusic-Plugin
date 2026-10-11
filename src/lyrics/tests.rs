use super::*;
use serde_json::json;
#[test]
fn yrcKeepsAbsoluteWordTimingTranslationAndMetadata() {
    let value = json!({"yrc":{"lyric":"{\"t\":0,\"c\":[{\"tx\":\"作曲：合成\"}]}\n[16210,1080](16210,670,0)还(16880,410,0)没"},"tlyric":{"lyric":"[00:16.210]Not yet"}});
    let lyrics = parse(&value).unwrap();
    let original = lyrics.Original.unwrap();
    assert_eq!(original.Lines[1].Text, "还没");
    assert_eq!(original.Lines[1].Words[1].StartMs, 16880);
    assert_eq!(original.Lines[1].Words[1].DurationMs, 410);
    assert_eq!(lyrics.Translation.unwrap().Lines[0].StartMs, Some(16210));
}
#[test]
fn lrcMultiTagsOffsetAndPlainTextAreNotInventedWordTiming() {
    let value = json!({"lrc":{"lyric":"[offset:-80]\n[00:01.5][00:03.050]合成歌词"}});
    let lyrics = parse(&value).unwrap();
    let timeline = lyrics.Original.unwrap();
    assert_eq!(timeline.OffsetMs, -80);
    assert_eq!(timeline.Lines[0].StartMs, Some(1500));
    assert_eq!(timeline.Lines[1].StartMs, Some(3050));
    assert!(timeline.Lines[0].Words.is_empty());
    assert!(parse(&json!({"nolyric":true})).unwrap().Original.is_none());
    assert_eq!(parse(&json!({})).unwrap_err().code, "LYRICS_UNAVAILABLE");
    assert!(parse(&json!({"lrc":{"lyric":"[00:60]bad"}})).is_err());
}
#[test]
fn malformedYrcCanUseActualLrcButNeverFakeWordTiming() {
    let lyrics =
        parse(&json!({"yrc":{"lyric":"[1,300](bad)bad"},"lrc":{"lyric":"[00:01]valid"}})).unwrap();
    assert!(lyrics.Original.unwrap().Lines[0].Words.is_empty());
    assert!(parse(&json!({"yrc":{"lyric":"[1,300](bad)bad"}})).is_err());
}
