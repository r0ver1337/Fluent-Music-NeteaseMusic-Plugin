use super::{crypto::*, testing::*, *};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ring::{aead, agreement, rand::SystemRandom};
use serde_json::{Value, json};
use std::io::Write;

fn decodeContent(body: &str, key: &[u8]) -> Value {
    let fields = form(body);
    assert!(fields.contains_key("S"));
    assert!(fields.contains_key("R"));
    let cipher = URL_SAFE_NO_PAD.decode(&fields["C"]).unwrap();
    let transformed = ecb(key, &cipher, false).unwrap();
    let mask = &transformed[..16];
    let mut xored = transformed[16..].to_vec();
    let rotate = (mask[0] as usize & 15) % xored.len();
    xored.rotate_right(rotate);
    for (index, byte) in xored.iter_mut().enumerate() {
        *byte ^= mask[index & 15];
    }
    let plain = ecb(staticKey, &xored, false).unwrap();
    serde_json::from_slice(&plain).unwrap()
}
#[test]
fn xeapiPublicKeySignatureX25519GcmEnvelopeSessionReuseAndGzipAreReal() {
    let peer =
        agreement::EphemeralPrivateKey::generate(&agreement::X25519, &SystemRandom::new()).unwrap();
    let peerPublic = STANDARD.encode(peer.compute_public_key().unwrap().as_ref());
    let mut peer = Some(peer);
    let mut call = 0;
    let server = serve(3, move |path, headers, body| {
        call += 1;
        if call == 1 {
            assert_eq!(path, "/api/bsr/sk/get");
            let fields = form(body);
            assert_eq!(fields["os"], "android");
            assert_eq!(
                fields["signature"],
                sign(&fields["timestamp"], &fields["nonce"])
            );
            let key = json!({"publicKey":peerPublic,"version":1,"sk":"synthetic-key-token"});
            let cipher = ecb(staticKey, &serde_json::to_vec(&key).unwrap(), true).unwrap();
            return json(
                &json!({"code":200,"data":{"timestamp":fields["timestamp"],"signature":sign(&fields["timestamp"],&fields["nonce"]),"encryptedData":STANDARD.encode(cipher)}}),
            );
        }
        assert_eq!(headers["x-os"], "android");
        assert_eq!(headers["x-client-enc-state"], "ENCRYPTED");
        let sessionKey = b"0123456789abcdefghijklmnopqrstuv";
        let key = if call == 2 {
            let fields = form(body);
            let s = STANDARD.decode(&fields["S"]).unwrap();
            let clientPublic = agreement::UnparsedPublicKey::new(&agreement::X25519, &s[..32]);
            let aes = agreement::agree_ephemeral(peer.take().unwrap(), &clientPublic, |shared| {
                let prk = hmac(&[0; 32], shared);
                let mut info = s[..32].to_vec();
                info.push(1);
                hmac(&prk, &info)[..16].to_vec()
            })
            .unwrap();
            let opening =
                aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_128_GCM, &aes).unwrap());
            let mut sealed = s[44..].to_vec();
            let plain = opening
                .open_in_place(
                    aead::Nonce::assume_unique_for_key(s[32..44].try_into().unwrap()),
                    aead::Aad::empty(),
                    &mut sealed,
                )
                .unwrap();
            let parts = std::str::from_utf8(plain)
                .unwrap()
                .split('|')
                .collect::<Vec<_>>();
            assert_eq!(parts[1], "android");
            assert_eq!(parts[2], "synthetic-key-token");
            assert_eq!(
                std::str::from_utf8(
                    &ecb(staticKey, &STANDARD.decode(&fields["R"]).unwrap(), false).unwrap()
                )
                .unwrap(),
                "1|"
            );
            STANDARD.decode(parts[0]).unwrap()
        } else {
            sessionKey.to_vec()
        };
        let value = decodeContent(body, &key);
        assert_eq!(value["queryString"], "e_r=true");
        let content = form(value["content"].as_str().unwrap());
        let reply = if call == 2 {
            assert_eq!(path, "/xeapi/register/anonimous");
            let encoded = STANDARD.decode(&content["username"]).unwrap();
            let decoded = std::str::from_utf8(&encoded).unwrap();
            assert_eq!(decoded.split_whitespace().next().unwrap().len(), 52);
            json!({"code":200})
        } else {
            assert_eq!(path, "/xeapi/song/enhance/player/url/v1");
            assert_eq!(content["level"], "standard");
            assert_eq!(content["ids"], "[123]");
            json!({"code":200,"data":[{"code":200,"id":123,"url":"https://m7.music.126.net/fixture.mp3","level":"standard","type":"mp3","freeTrialInfo":null}]})
        };
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&serde_json::to_vec(&reply).unwrap())
            .unwrap();
        let bytes = ecb(eapiKey, &gzip.finish().unwrap(), true).unwrap();
        let headers = if call == 2 {
            vec![
                (
                    "Set-Cookie".into(),
                    "MUSIC_A=synthetic-guest; Secure; Path=/".into(),
                ),
                ("x-encr-ssid".into(), "synthetic-session".into()),
                (
                    "x-encr-sskey".into(),
                    String::from_utf8(sessionKey.to_vec()).unwrap(),
                ),
            ]
        } else {
            Vec::new()
        };
        (headers, bytes)
    });
    let mut client = Client::new(Device::generate().unwrap()).unwrap();
    client.testOrigin(&server.Origin);
    let mut cookies = Cookies::default();
    client.guest(&mut cookies).unwrap();
    let value = client
        .request(
            "/api/song/enhance/player/url/v1",
            json!({"ids":"[123]","level":"standard","encodeType":"flac"}),
            Crypto::Xeapi,
            &mut cookies,
            &[200],
        )
        .unwrap();
    crate::media::playback(&value, "123", "standard").unwrap();
    server.finish();
}
