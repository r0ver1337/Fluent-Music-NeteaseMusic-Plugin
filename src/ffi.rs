use crate::{protocol::fail, provider::Provider};
use fluent_plugin_abi::*;
use std::{sync::Mutex, time::Duration};

unsafe extern "C" fn createProvider(
    host: *const fmHostApi,
    info: *const fmCreateInfo,
    out: *mut fmHandle,
) -> fmStatus {
    if out.is_null() {
        return -1;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    if info.is_null() {
        return -1;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if unsafe { (*info).structSize } < size_of::<fmCreateInfo>() as u32 {
            return Err(());
        }
        if unsafe { decode::text((*info).pluginId) }.map_err(|_| ())? != "netease" {
            return Err(());
        }
        unsafe {
            io::installApi(host).map_err(|_| ())?;
        }
        Provider::broker().map_err(|_| ())
    }));
    match result {
        Ok(Ok(provider)) => {
            unsafe {
                *out = Box::into_raw(Box::new(Mutex::new(provider))).cast();
            }
            0
        }
        _ => -1,
    }
}
unsafe extern "C" fn destroyProvider(handle: fmHandle) -> fmStatus {
    if handle.is_null() {
        return -1;
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(handle.cast::<Mutex<Provider>>()));
    })) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
unsafe extern "C" fn call(
    handle: fmHandle,
    context: *const fmCallContext,
    request: *const fmRequest,
    out: *mut fmHandle,
) -> fmStatus {
    if out.is_null() {
        return -1;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    if handle.is_null() || context.is_null() {
        return -1;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if unsafe { (*context).structSize } < size_of::<fmCallContext>() as u32 {
            return Err(fail("ABI", "网易云调用上下文无效"));
        }
        let context = unsafe { *context };
        let cancelled = || {
            context
                .isCancelled
                .is_some_and(|check| unsafe { check(context.userData) } != 0)
        };
        if cancelled() {
            return Err(fail("CANCELLED", "网易云请求已取消"));
        }
        if context.timeoutMs == 0 {
            return Err(fail("TIMEOUT", "网易云请求预算已耗尽"));
        }
        let operation = unsafe { decodeRequest(request)? };
        let mut provider = unsafe { &*handle.cast::<Mutex<Provider>>() }
            .lock()
            .map_err(|_| fail("ABI", "网易云插件实例已失效"))?;
        provider.handleControlled(
            operation,
            Duration::from_millis(context.timeoutMs.min(80000)),
            &cancelled,
        )
    }))
    .unwrap_or_else(|_| Err(fail("ABI", "网易云插件发生内部错误")));
    unsafe {
        *out = OwnedResult::new(result).intoHandle();
    }
    0
}
unsafe extern "C" fn getResultView(handle: fmHandle) -> *const fmResultView {
    if handle.is_null() {
        return std::ptr::null();
    }
    unsafe { &(*handle.cast::<OwnedResult>()).View }
}
unsafe extern "C" fn releaseResult(handle: fmHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle.cast::<OwnedResult>()));
        }
    }
}
static pluginApi: fmPluginApi = fmPluginApi {
    structSize: size_of::<fmPluginApi>() as u32,
    abiVersion,
    createProvider,
    destroyProvider,
    describe: Some(call),
    loginBegin: Some(call),
    loginPoll: Some(call),
    logout: Some(call),
    getAccount: Some(call),
    search: Some(call),
    listPlaylists: Some(call),
    getPlaylist: Some(call),
    getRecommendations: Some(call),
    getHomepage: Some(call),
    getPlaylistRecommendations: Some(call),
    resolveTrack: Some(call),
    getResultView,
    releaseResult,
    getLyrics: Some(call),
    getTrackQualities: Some(call),
    resolveTrackWithQuality: Some(call),
};

/// # Safety
/// `out` must point to writable pointer storage owned by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmPluginGetApi(version: u32, out: *mut *const fmPluginApi) -> fmStatus {
    if out.is_null() {
        return -1;
    }
    unsafe {
        *out = std::ptr::null();
    }
    if version != abiVersion {
        return -2;
    }
    unsafe {
        *out = &pluginApi;
    }
    0
}
