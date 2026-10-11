#![cfg(windows)]
#![allow(non_snake_case)]
use fluent_plugin_api::*;
use fluent_plugin_host::PluginProcess;
use std::path::{Path, PathBuf};
fn binaryDirectory() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
fn install(runtime: &Path) -> PathBuf {
    let plugin = runtime.join("plugins/netease");
    std::fs::create_dir_all(&plugin).unwrap();
    let binary = binaryDirectory();
    let candidates = [binary.join("deps/netease.dll"), binary.join("netease.dll")];
    std::fs::copy(
        candidates
            .iter()
            .find(|path| path.is_file())
            .expect("Build netease DLL first"),
        plugin.join("netease.dll"),
    )
    .unwrap();
    let path = plugin.join("plugin.json");
    std::fs::write(&path, include_bytes!("../plugin.json")).unwrap();
    path
}
#[test]
fn actualDllAppContainerHandshakeFileApiDpapiAndLogoutNeedNoNetwork() {
    let base = std::env::temp_dir().join("opencode");
    std::fs::create_dir_all(&base).unwrap();
    let runtime = tempfile::Builder::new()
        .prefix("netease-dll-")
        .tempdir_in(base)
        .unwrap();
    let manifest = install(runtime.path());
    let worker = binaryDirectory().join("pluginloader.exe");
    assert!(worker.is_file(), "Build pluginloader before DLL tests");
    let mut plugin =
        PluginProcess::startWithRuntime(&manifest, runtime.path(), &worker, true).unwrap();
    assert_eq!(plugin.info.id, "netease");
    assert_eq!(plugin.info.name, "网易云音乐");
    for capability in [
        Capability::Login,
        Capability::Account,
        Capability::Search,
        Capability::Playlists,
        Capability::Playlist,
        Capability::Resolve,
        Capability::Lyrics,
        Capability::TrackQualities,
    ] {
        assert!(plugin.info.supports(&capability));
    }
    assert_eq!(
        plugin.call(Operation::Account).err().unwrap().code,
        "LOGIN_REQUIRED"
    );
    assert_eq!(
        plugin
            .call(Operation::Search {
                query: String::new(),
                page: 0
            })
            .err()
            .unwrap()
            .code,
        "PARAMETERS"
    );
    let ResultData::TrackQualities(list) = plugin
        .call(Operation::TrackQualities { track: None })
        .unwrap()
    else {
        panic!()
    };
    list.validate().unwrap();
    plugin.call(Operation::Logout).unwrap();
    drop(plugin);
    let identity = runtime.path().join("data/netease/device.dat");
    let encrypted = std::fs::read(&identity).unwrap();
    assert!(encrypted.starts_with(b"FMNETEASE\x01"));
    assert!(!encrypted.windows(8).any(|window| window == b"deviceId"));
    let session = std::fs::read(runtime.path().join("data/netease/session.dat")).unwrap();
    assert!(session.starts_with(b"FMNETEASE\x01"));
    assert!(!session.windows(7).any(|window| window == b"MUSIC_U"));
    let mut plugin =
        PluginProcess::startWithRuntime(&manifest, runtime.path(), &worker, true).unwrap();
    assert_eq!(
        plugin.call(Operation::Account).err().unwrap().code,
        "LOGIN_REQUIRED"
    );
    drop(plugin);
    assert_eq!(std::fs::read(identity).unwrap(), encrypted);
}
#[test]
fn abiTableIsTypedAppendOnlyAndRejectsWrongVersion() {
    let mut pointer = std::ptr::null();
    assert_eq!(unsafe { netease::fmPluginGetApi(2, &mut pointer) }, 0);
    let api = unsafe { fluent_plugin_abi::readPluginApi(pointer) }.unwrap();
    assert!(api.resolveTrack.is_some());
    assert!(api.resolveTrackWithQuality.is_some());
    assert!(api.listPlaylists.is_some());
    assert!(api.getLyrics.is_some());
    assert_eq!(unsafe { netease::fmPluginGetApi(1, &mut pointer) }, -2);
    assert!(pointer.is_null());
    assert_eq!(
        unsafe { netease::fmPluginGetApi(1, std::ptr::null_mut()) },
        -1
    );
}
