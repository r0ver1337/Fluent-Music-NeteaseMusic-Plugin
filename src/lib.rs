#![allow(non_snake_case, non_upper_case_globals)]

mod ffi;
mod lyrics;
mod media;
mod metadata;
mod protocol;
mod provider;
mod sessionStore;
pub use ffi::fmPluginGetApi;
#[cfg(test)]
mod mediaTests;
#[cfg(test)]
mod metadataTests;
#[cfg(not(test))]
use fluent_plugin_abi::network::http;
#[cfg(test)]
use reqwest::blocking as http;
