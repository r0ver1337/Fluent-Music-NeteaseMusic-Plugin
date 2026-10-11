use super::storageError;
use fluent_plugin_api::ApiError;

#[cfg(windows)]
fn dpapi(input: &[u8], encrypt: bool) -> Result<Vec<u8>, ApiError> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    use zeroize::Zeroize;
    let entropyBytes = b"FluentMusic/netease/credentials/v1";
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(input.len()).map_err(|_| storageError("网易云加密记录过大"))?,
        pbData: input.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropyBytes.len() as u32,
        pbData: entropyBytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(storageError(if encrypt {
            "Windows 无法加密网易云记录"
        } else {
            "Windows 无法解密网易云记录，请在原用户下重新授权"
        }));
    }
    let result = unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = bytes.to_vec();
        bytes.zeroize();
        LocalFree(output.pbData.cast());
        result
    };
    Ok(result)
}
pub(super) fn protect(input: &[u8]) -> Result<Vec<u8>, ApiError> {
    #[cfg(windows)]
    {
        dpapi(input, true)
    }
    #[cfg(not(windows))]
    {
        let _ = input;
        Err(storageError("当前平台不支持安全网易云凭据存储"))
    }
}
pub(super) fn unprotect(input: &[u8]) -> Result<Vec<u8>, ApiError> {
    #[cfg(windows)]
    {
        dpapi(input, false)
    }
    #[cfg(not(windows))]
    {
        let _ = input;
        Err(storageError("当前平台不支持安全网易云凭据存储"))
    }
}
