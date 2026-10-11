use super::fail;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit, generic_array::GenericArray};
use base64::{Engine, engine::general_purpose::STANDARD};
use cbc::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use fluent_plugin_api::ApiError;
use md5::{Digest, Md5};
use num_bigint::BigUint;
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::Value;

pub(super) const eapiKey: &[u8; 16] = b"e82ckenh8dichen8";
pub(super) const staticKey: &[u8; 32] = &[
    0xab, 0x1d, 0x5a, 0x43, 0x0f, 0x6b, 0xb0, 0x4a, 0x3f, 0x01, 0xe8, 0x1d, 0xdd, 0x72, 0xbd, 0x91,
    0x6d, 0x5c, 0xe5, 0x91, 0x24, 0x8a, 0xc1, 0x28, 0x71, 0x48, 0x06, 0xd7, 0xf8, 0xfb, 0x1b, 0x84,
];
pub(crate) fn random<const N: usize>() -> Result<[u8; N], ApiError> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| fail("CRYPTO", "安全随机数不可用"))?;
    Ok(bytes)
}
pub(crate) fn md5(bytes: &[u8]) -> Vec<u8> {
    Md5::digest(bytes).to_vec()
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}
pub(super) fn hmac(key: &[u8], bytes: &[u8]) -> Vec<u8> {
    hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), bytes)
        .as_ref()
        .to_vec()
}
pub(super) fn sign(timestamp: &str, nonce: &str) -> String {
    STANDARD.encode(hmac(
        b"mUHCwVNWJbunMqAHf5MImuirT6plvs6VSFW62MGHstFQxhBGdEoIhLItH3djc4+FB/OKty3+lL2rGeoFBpVe5g==",
        format!("{timestamp}{nonce}").as_bytes(),
    ))
}
pub(super) fn ecb(key: &[u8], input: &[u8], encrypt: bool) -> Result<Vec<u8>, ApiError> {
    let error = || fail("CRYPTO", "网易云加密数据无效");
    let mut bytes = input.to_vec();
    if encrypt {
        let pad = 16 - bytes.len() % 16;
        bytes.resize(bytes.len() + pad, pad as u8);
    } else if bytes.is_empty() || !bytes.len().is_multiple_of(16) {
        return Err(error());
    }
    for chunk in bytes.as_chunks_mut::<16>().0 {
        let block = GenericArray::from_mut_slice(chunk);
        match key.len() {
            16 => {
                let cipher = aes::Aes128::new_from_slice(key).map_err(|_| error())?;
                if encrypt {
                    cipher.encrypt_block(block)
                } else {
                    cipher.decrypt_block(block)
                }
            }
            32 => {
                let cipher = aes::Aes256::new_from_slice(key).map_err(|_| error())?;
                if encrypt {
                    cipher.encrypt_block(block)
                } else {
                    cipher.decrypt_block(block)
                }
            }
            24 => {
                let cipher = aes::Aes192::new_from_slice(key).map_err(|_| error())?;
                if encrypt {
                    cipher.encrypt_block(block)
                } else {
                    cipher.decrypt_block(block)
                }
            }
            _ => return Err(error()),
        }
    }
    if !encrypt {
        let pad = *bytes.last().ok_or_else(error)? as usize;
        if pad == 0
            || pad > 16
            || !bytes[bytes.len() - pad..]
                .iter()
                .all(|byte| *byte as usize == pad)
        {
            return Err(error());
        }
        bytes.truncate(bytes.len() - pad);
    }
    Ok(bytes)
}
pub(super) fn eapi(uri: &str, data: &Value) -> Result<String, ApiError> {
    let text = serde_json::to_string(data).map_err(|_| fail("CRYPTO", "请求编码失败"))?;
    let digest = hex(&md5(format!("nobody{uri}use{text}md5forencrypt").as_bytes())).to_lowercase();
    let plain = format!("{uri}-36cd479b6b5-{text}-36cd479b6b5-{digest}");
    Ok(hex(&ecb(eapiKey, plain.as_bytes(), true)?))
}
fn cbcEncrypt(input: &[u8], key: &[u8; 16]) -> String {
    STANDARD.encode(
        cbc::Encryptor::<aes::Aes128>::new(key.into(), b"0102030405060708".into())
            .encrypt_padded_vec_mut::<Pkcs7>(input),
    )
}
pub(super) fn weapiWithKey(data: &Value, key: &[u8; 16]) -> Result<(String, String), ApiError> {
    let text = serde_json::to_vec(data).map_err(|_| fail("CRYPTO", "请求编码失败"))?;
    let first = cbcEncrypt(&text, b"0CoJUm6Qyw8W8jud");
    let params = cbcEncrypt(first.as_bytes(), key);
    let der = STANDARD.decode("MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ37BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvaklV8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44oncaTWz7OBGLbCiK45wIDAQAB").map_err(|_| fail("CRYPTO", "协议公钥无效"))?;
    let modulus = BigUint::from_bytes_be(&der[29..157]);
    let reversed: Vec<u8> = key.iter().rev().copied().collect();
    let result = BigUint::from_bytes_be(&reversed).modpow(&BigUint::from(65537u32), &modulus);
    Ok((params, format!("{:0>256}", result.to_str_radix(16))))
}
pub(super) fn weapi(data: &Value) -> Result<(String, String), ApiError> {
    let alphabet = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut key = [0; 16];
    for slot in &mut key {
        loop {
            let byte = random::<1>()?[0];
            if byte < 248 {
                *slot = alphabet[(byte % 62) as usize];
                break;
            }
        }
    }
    weapiWithKey(data, &key)
}

#[cfg(test)]
mod tests;
