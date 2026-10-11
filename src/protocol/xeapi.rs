use super::{crypto::*, fail};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use fluent_plugin_api::ApiError;
use ring::{aead, agreement, rand::SystemRandom};
use serde_json::{Value, json};
use zeroize::Zeroizing;

pub(super) struct PublicKey {
    pub Peer: Vec<u8>,
    pub Version: String,
    pub Sk: Zeroizing<String>,
}
impl PublicKey {
    pub fn parse(value: &Value) -> Result<Self, ApiError> {
        let error = || fail("CRYPTO", "网易云 xeapi 公钥数据无效");
        let peer = STANDARD
            .decode(value["publicKey"].as_str().ok_or_else(error)?)
            .map_err(|_| error())?;
        let version = match &value["version"] {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            _ => return Err(error()),
        };
        let sk = value["sk"].as_str().ok_or_else(error)?;
        if peer.len() != 32
            || version.is_empty()
            || version.len() > 64
            || sk.is_empty()
            || sk.len() > 4096
        {
            return Err(error());
        }
        Ok(Self {
            Peer: peer,
            Version: version,
            Sk: Zeroizing::new(sk.to_owned()),
        })
    }
}
pub(super) fn encrypt(
    data: &Value,
    key: &PublicKey,
    sessionId: &str,
    sessionKey: &str,
) -> Result<Vec<(String, String)>, ApiError> {
    let error = || fail("CRYPTO", "网易云 xeapi 请求加密失败");
    let dynamic = Zeroizing::new(if sessionKey.is_empty() {
        random::<16>()?.to_vec()
    } else {
        sessionKey.as_bytes().to_vec()
    });
    if !matches!(dynamic.len(), 16 | 24 | 32) {
        return Err(error());
    }
    let content = super::client::formData(data)?;
    let text = Zeroizing::new(
        serde_json::to_vec(&json!({"content": content, "queryString": "e_r=true"}))
            .map_err(|_| error())?,
    );
    let inner = ecb(staticKey, &text, true)?;
    let mask = random::<16>()?;
    let mut xored: Vec<_> = inner
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ mask[index & 15])
        .collect();
    let rotate = (mask[0] as usize & 15) % xored.len();
    xored.rotate_left(rotate);
    let mut transformed = mask.to_vec();
    transformed.extend_from_slice(&xored);
    let c = ecb(&dynamic, &transformed, true)?;
    let ephemeral =
        agreement::EphemeralPrivateKey::generate(&agreement::X25519, &SystemRandom::new())
            .map_err(|_| error())?;
    let public = ephemeral.compute_public_key().map_err(|_| error())?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::X25519, &key.Peer);
    let aesKey = agreement::agree_ephemeral(ephemeral, &peer, |shared| {
        let prk = hmac(&[0; 32], shared);
        let mut info = public.as_ref().to_vec();
        info.push(1);
        hmac(&prk, &info)[..16].to_vec()
    })
    .map_err(|_| error())?;
    let sealing = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_128_GCM, &aesKey).map_err(|_| error())?,
    );
    let iv = random::<12>()?;
    let mut sealed =
        format!("{}|android|{}", STANDARD.encode(&dynamic), key.Sk.as_str()).into_bytes();
    sealing
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(iv),
            aead::Aad::empty(),
            &mut sealed,
        )
        .map_err(|_| error())?;
    let mut s = public.as_ref().to_vec();
    s.extend_from_slice(&iv);
    s.extend_from_slice(&sealed);
    let r = ecb(
        staticKey,
        format!(
            "{}|{}",
            key.Version,
            if sessionKey.is_empty() { "" } else { sessionId }
        )
        .as_bytes(),
        true,
    )?;
    Ok(vec![
        ("C".into(), URL_SAFE_NO_PAD.encode(c)),
        ("S".into(), STANDARD.encode(s)),
        ("R".into(), STANDARD.encode(r)),
    ])
}
