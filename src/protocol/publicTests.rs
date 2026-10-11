use super::*;
use serde_json::json;

#[test]
#[ignore = "Explicit fresh-guest public network permission required"]
fn freshGuestOfficialBootstrapAndPublicMetadata() {
    assert_eq!(
        std::env::var("FLUENT_NETEASE_PUBLIC_TEST").as_deref(),
        Ok("1")
    );
    let mut client = Client::new(Device::generate().unwrap()).unwrap();
    let mut cookies = Cookies::default();
    client.guest(&mut cookies).expect("fresh guest bootstrap");
    assert!(cookies.guest());
    assert!(!cookies.authorized());
    eprintln!("fresh guest bootstrap: verified");
    let search = client
        .request(
            "/api/cloudsearch/pc",
            json!({"s":"贝多芬", "type":1,"limit":30,"offset":0,"total":true}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )
        .expect("public search");
    let songs = search["result"]["songs"].as_array().expect("search songs");
    assert!(!songs.is_empty());
    eprintln!("public search: {} results", songs.len());
    let recommendation = client
        .request(
            "/api/personalized/playlist",
            json!({"limit":10,"total":true,"n":1000}),
            Crypto::Weapi,
            &mut cookies,
            &[200],
        )
        .expect("public playlist recommendations");
    let list = crate::metadata::playlists(&recommendation["result"])
        .expect("real public playlist parsing");
    assert!(!list.is_empty());
    let detail = client
        .request(
            "/api/v6/playlist/detail",
            json!({"id":list[0].id,"n":100000,"s":0}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )
        .expect("public playlist full ID snapshot");
    let ids = detail["playlist"]["trackIds"]
        .as_array()
        .expect("public track IDs");
    assert!(!ids.is_empty());
    let c = ids
        .iter()
        .take(50)
        .map(|item| json!({"id":item["id"]}))
        .collect::<Vec<_>>();
    let songsDetail = client
        .request(
            "/api/v3/song/detail",
            json!({"c":serde_json::to_string(&c).unwrap()}),
            Crypto::Weapi,
            &mut cookies,
            &[200],
        )
        .expect("public playlist detail batch");
    assert_eq!(
        crate::metadata::tracks(&songsDetail["songs"])
            .expect("real playlist track parsing")
            .len(),
        c.len()
    );
    eprintln!(
        "public playlist: complete ID snapshot and {} metadata records verified",
        c.len()
    );
    let newsongs = client
        .request(
            "/api/personalized/newsong",
            json!({"type":"recommend","limit":10,"areaId":0}),
            Crypto::Weapi,
            &mut cookies,
            &[200],
        )
        .expect("public new song recommendations");
    assert!(
        !crate::metadata::tracks(&newsongs["result"])
            .expect("real new song parsing")
            .is_empty()
    );
    eprintln!("public weapi recommendations: verified");
    let song = songs
        .iter()
        .find(|song| song["fee"].as_u64() == Some(0))
        .expect("a free public result");
    let id = song["id"].as_u64().unwrap();
    let lyric = client
        .request(
            "/api/song/lyric/v1",
            json!({"id":id,"cp":false,"tv":0,"lv":0,"rv":0,"kv":0,"yv":0,"ytv":0,"yrv":0}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )
        .expect("public lyric");
    match crate::lyrics::parse(&lyric) {
        Ok(value) => value.validate().unwrap(),
        Err(error) if error.code == "LYRICS_UNAVAILABLE" => {
            eprintln!("public instrumental lyric: upstream unavailable")
        }
        Err(error) => panic!("real lyric parsing failed: {error}"),
    }
    eprintln!("public lyric: verified");
    let playback = client
        .request(
            "/api/song/enhance/player/url/v1",
            json!({"ids":format!("[{id}]"),"level":"standard","encodeType":"flac"}),
            Crypto::Xeapi,
            &mut cookies,
            &[200],
        )
        .expect("public standard playback authorization");
    let item = &playback["data"][0];
    eprintln!(
        "public playback: code={}, urlPresent={}, trial={}, type={}",
        item["code"],
        item["url"].is_string(),
        item["freeTrialInfo"].is_object(),
        item["type"]
    );
    assert!(
        item["url"].is_string(),
        "server must return a legal public URL"
    );
    let (source, _) = crate::media::playback(&playback, &id.to_string(), "standard")
        .expect("full-song permission and domain validation");
    decodePublicAudio(source);
}

fn decodePublicAudio(source: fluent_plugin_api::MediaSource) {
    use std::io::{Cursor, Read};
    use symphonia::core::{
        codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
        meta::MetadataOptions, probe::Hint,
    };
    let fluent_plugin_api::MediaSource::Http { url, headers } = source else {
        panic!()
    };
    assert!(headers.is_empty());
    let response = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap()
        .get(url)
        .header("Range", "bytes=0-262143")
        .send()
        .expect("minimal authorized public audio download");
    assert!(response.status().is_success());
    let contentType = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    assert!(!contentType.contains("text/html"));
    let mut bytes = Vec::new();
    response.take(262144).read_to_end(&mut bytes).unwrap();
    assert!(!bytes.is_empty());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .expect("real public audio probe")
        .format;
    let track = format.default_track().expect("audio track");
    let trackId = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .expect("audio decoder");
    let mut frames = 0;
    for _ in 0..64 {
        let packet = format.next_packet().expect("audio packet");
        if packet.track_id() != trackId {
            continue;
        }
        if let Ok(buffer) = decoder.decode(&packet) {
            frames += buffer.frames();
        }
        if frames > 0 {
            break;
        }
    }
    assert!(frames > 0);
    eprintln!(
        "authorized public MP3: decoded {frames} real audio frames, contentType={contentType}"
    );
}

#[test]
#[ignore = "Explicit fresh-guest public artwork permission required"]
fn publicArtworkMetadataUsesHttpsAndActuallyDecodes() {
    use std::io::Read;
    assert_eq!(
        std::env::var("FLUENT_NETEASE_PUBLIC_TEST").as_deref(),
        Ok("1")
    );
    let mut client = Client::new(Device::generate().unwrap()).unwrap();
    let mut cookies = Cookies::default();
    client.guest(&mut cookies).expect("fresh guest bootstrap");
    let search = client
        .request(
            "/api/cloudsearch/pc",
            json!({"s":"贝多芬","type":1,"limit":30,"offset":0,"total":true}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )
        .expect("public search metadata");
    let songs = search["result"]["songs"].as_array().expect("public songs");
    let urls: Vec<_> = songs
        .iter()
        .filter_map(|song| song.get("al").or_else(|| song.get("album"))?["picUrl"].as_str())
        .collect();
    let httpCount = urls.iter().filter(|url| url.starts_with("http://")).count();
    let tracks = crate::metadata::tracks(&search["result"]["songs"]).unwrap();
    let coverCount = tracks
        .iter()
        .filter(|track| track.cover_url.is_some())
        .count();
    eprintln!(
        "public artwork: {} songs, {} provided album URLs, {} HTTP URLs, {} mapped covers",
        tracks.len(),
        urls.len(),
        httpCount,
        coverCount
    );
    assert!(!urls.is_empty(), "Search must contain real album artwork");
    for raw in &urls {
        if raw.starts_with("http://") {
            assert!(
                crate::metadata::artwork(&json!(raw)).is_some(),
                "Official HTTP cover was discarded"
            );
        }
    }
    let cover = tracks
        .iter()
        .find_map(|track| track.cover_url.as_ref())
        .expect("mapped song cover");
    assert!(cover.starts_with("https://"));
    let response = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap()
        .get(cover)
        .send()
        .expect("public HTTPS cover download");
    assert!(response.status().is_success());
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 2 * 1024 * 1024);
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .unwrap();
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().expect("real cover decode");
    assert!(image.width() > 0 && image.height() > 0);
    eprintln!(
        "public HTTPS artwork decoded: {}x{}, {} bytes",
        image.width(),
        image.height(),
        bytes.len()
    );
}
