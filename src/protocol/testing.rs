use super::{Client, crypto};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
    time::Duration,
};

pub(crate) struct Server {
    pub Origin: String,
    Worker: JoinHandle<()>,
}
impl Server {
    pub fn finish(self) {
        self.Worker.join().unwrap();
    }
}
pub(crate) fn serve<F>(count: usize, mut handler: F) -> Server
where
    F: FnMut(&str, &BTreeMap<String, String>, &str) -> (Vec<(String, String)>, Vec<u8>)
        + Send
        + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        for _ in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            let (headerEnd, length, headers) = loop {
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&chunk[..size]);
                assert!(bytes.len() < 1024 * 1024);
                if let Some(end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let header = std::str::from_utf8(&bytes[..end]).unwrap();
                    let headers: BTreeMap<_, _> = header
                        .lines()
                        .skip(1)
                        .map(|line| {
                            let (key, value) = line.split_once(':').unwrap();
                            (key.to_ascii_lowercase(), value.trim().to_owned())
                        })
                        .collect();
                    let length = headers["content-length"].parse::<usize>().unwrap();
                    break (end + 4, length, headers);
                }
            };
            while bytes.len() < headerEnd + length {
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&chunk[..size]);
            }
            let path = std::str::from_utf8(&bytes[..headerEnd])
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap();
            let body = std::str::from_utf8(&bytes[headerEnd..headerEnd + length]).unwrap();
            assert!(!headers.contains_key("x-real-ip"));
            assert!(!headers.contains_key("x-forwarded-for"));
            let (headers, response) = handler(path, &headers, body);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                response.len()
            )
            .unwrap();
            for (key, value) in headers {
                write!(stream, "{key}: {value}\r\n").unwrap();
            }
            stream.write_all(b"\r\n").unwrap();
            stream.write_all(&response).unwrap();
        }
    });
    Server {
        Origin: origin,
        Worker: worker,
    }
}
pub(crate) fn form(body: &str) -> BTreeMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}
pub(crate) fn decodeEapi(body: &str) -> (String, Value) {
    let params = &form(body)["params"];
    let bytes = (0..params.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&params[index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let bytes = crypto::ecb(crypto::eapiKey, &bytes, false).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let parts = text.split("-36cd479b6b5-").collect::<Vec<_>>();
    assert_eq!(parts.len(), 3);
    assert_eq!(
        parts[2],
        crypto::hex(&crypto::md5(
            format!("nobody{}use{}md5forencrypt", parts[0], parts[1]).as_bytes()
        ))
        .to_lowercase()
    );
    (parts[0].into(), serde_json::from_str(parts[1]).unwrap())
}
pub(crate) fn json(value: &Value) -> (Vec<(String, String)>, Vec<u8>) {
    (Vec::new(), serde_json::to_vec(value).unwrap())
}
impl Client {
    pub(crate) fn testOrigin(&mut self, origin: &str) {
        let url = url::Url::parse(origin).unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        self.TestOrigin = Some(origin.into());
        self.deterministicWeapi();
    }
}
pub(crate) fn decodeWeapi(body: &str) -> Value {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use cbc::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    let fields = form(body);
    let second = STANDARD.decode(&fields["params"]).unwrap();
    let first =
        cbc::Decryptor::<aes::Aes128>::new(b"abcdefghijklmnop".into(), b"0102030405060708".into())
            .decrypt_padded_vec_mut::<Pkcs7>(&second)
            .unwrap();
    let first = STANDARD.decode(first).unwrap();
    let plain =
        cbc::Decryptor::<aes::Aes128>::new(b"0CoJUm6Qyw8W8jud".into(), b"0102030405060708".into())
            .decrypt_padded_vec_mut::<Pkcs7>(&first)
            .unwrap();
    serde_json::from_slice(&plain).unwrap()
}
