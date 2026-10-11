mod authentication;
mod discovery;
mod playback;
mod playlists;
mod state;
use crate::{
    metadata,
    protocol::{Client, Cookies, Crypto, Session, fail},
    sessionStore::SessionStore,
};
use fluent_plugin_api::*;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

struct Pending {
    Id: String,
    Key: Zeroizing<String>,
    Revision: String,
    Cookies: Cookies,
    Expires: Instant,
    NextPoll: Instant,
    State: LoginState,
}
struct PlaylistSnapshot {
    Id: String,
    Key: String,
    Revision: String,
    Tracks: Vec<String>,
    Expires: Instant,
}
pub(crate) struct Provider {
    Store: SessionStore,
    Client: Option<Client>,
    Session: Option<Session>,
    Guest: Cookies,
    Revision: Option<String>,
    Pending: Option<Pending>,
    Playlists: HashMap<String, PlaylistSnapshot>,
    Restored: bool,
}
impl Provider {
    pub fn broker() -> Result<Self, ApiError> {
        Ok(Self::withStore(SessionStore::broker()?))
    }
    fn withStore(store: SessionStore) -> Self {
        Self {
            Store: store,
            Client: None,
            Session: None,
            Guest: Cookies::default(),
            Revision: None,
            Pending: None,
            Playlists: HashMap::new(),
            Restored: false,
        }
    }
    pub fn handleControlled(
        &mut self,
        operation: Operation,
        timeout: Duration,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ResultData, ApiError> {
        if cancelled() {
            return Err(fail("CANCELLED", "网易云请求已取消"));
        }
        if matches!(operation, Operation::Describe) {
            return Ok(ResultData::Description(description()));
        }
        if matches!(operation, Operation::TrackQualities { track: None }) {
            return Ok(ResultData::TrackQualities(crate::media::catalogue()));
        }
        self.validateOperation(&operation)?;
        self.ensureClient()?;
        self.client().Deadline = Instant::now() + timeout.min(Duration::from_secs(80));
        if matches!(operation, Operation::Logout) {
            self.Session = None;
            self.Pending = None;
            self.Playlists.clear();
            self.client().resetTransport();
            self.Restored = true;
            self.Revision = Some(self.Store.clear(&self.Client.as_ref().unwrap().Device)?);
            return Ok(ResultData::Ok);
        }
        self.restore()?;
        let result = match operation {
            Operation::LoginBegin => self.loginBegin(),
            Operation::LoginPoll { challenge_id } => self.loginPoll(&challenge_id, cancelled),
            Operation::Account => self.account().map(ResultData::Account),
            Operation::Search { query, page } => self.search(&query, page).map(ResultData::Tracks),
            Operation::Playlists => self.library().map(ResultData::Playlists),
            Operation::Playlist { id, page, cursor } => self
                .playlist(&id, page, cursor.as_deref())
                .map(ResultData::Tracks),
            Operation::Recommendations => {
                let feed = self.feed()?;
                if feed.track_source == RecommendationSource::Unknown {
                    return Err(feed
                        .issues
                        .into_iter()
                        .find(|issue| issue.stage.ends_with("Tracks"))
                        .map(|issue| issue.error)
                        .unwrap_or_else(|| fail("RECOMMENDATIONS", "网易云未提供推荐歌曲")));
                }
                Ok(ResultData::Tracks(Page {
                    items: feed.tracks,
                    page: 0,
                    has_more: false,
                    cursor: None,
                }))
            }
            Operation::Homepage | Operation::PlaylistRecommendations => {
                self.feed().map(ResultData::RecommendationFeed)
            }
            Operation::Resolve { track } => self.resolve(&track, "standard").map(ResultData::Media),
            Operation::ResolveWithQuality { track, qualityId } => {
                self.resolve(&track, &qualityId).map(ResultData::Media)
            }
            Operation::TrackQualities { track: Some(track) } => {
                self.qualities(&track).map(ResultData::TrackQualities)
            }
            Operation::Lyrics { track } => {
                let value = self.request("/api/song/lyric/v1", json!({"id":track.id,"cp":false,"tv":0,"lv":0,"rv":0,"kv":0,"yv":0,"ytv":0,"yrv":0}), Crypto::Eapi)?;
                crate::lyrics::parse(&value).map(ResultData::Lyrics)
            }
            _ => unreachable!(),
        }?;
        if cancelled() {
            return Err(fail("CANCELLED", "网易云请求已取消"));
        }
        let size = serde_json::to_vec(&result)
            .map_err(|_| fail("SCHEMA", "网易云通用结果编码失败"))?
            .len();
        if size > MAX_MESSAGE_BYTES - 512 {
            return Err(fail("RESPONSE_LIMIT", "网易云结果超过插件传输限制"));
        }
        Ok(result)
    }
    fn validateOperation(&self, operation: &Operation) -> Result<(), ApiError> {
        match operation {
            Operation::Search { query, page }
                if query.trim().is_empty() || query.len() > 1024 || *page > 100000 =>
            {
                Err(fail("PARAMETERS", "搜索词或页码无效"))
            }
            Operation::Playlist { id, page, cursor } => {
                metadata::validateId(id)?;
                if *page > 2000 || cursor.as_ref().is_some_and(|cursor| cursor.len() > 128) {
                    return Err(fail("PARAMETERS", "网易云歌单页码或游标无效"));
                }
                Ok(())
            }
            Operation::Resolve { track }
            | Operation::Lyrics { track }
            | Operation::TrackQualities { track: Some(track) } => metadata::validateTrack(track),
            Operation::ResolveWithQuality { track, qualityId } => {
                metadata::validateTrack(track)?;
                crate::media::validateQuality(qualityId)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
fn description() -> PluginInfo {
    PluginInfo {
        id: "netease".into(),
        name: "网易云音乐".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        api_version: API_VERSION,
        capabilities: vec![
            Capability::Login,
            Capability::Account,
            Capability::Search,
            Capability::Playlists,
            Capability::Playlist,
            Capability::Recommendations,
            Capability::Resolve,
        ],
        extensions: vec![
            Capability::Homepage,
            Capability::PlaylistRecommendations,
            Capability::Lyrics,
            Capability::TrackQualities,
        ],
    }
}
#[cfg(all(test, windows))]
mod discoveryTests;
#[cfg(all(test, windows))]
mod loginTests;
#[cfg(all(test, windows))]
mod paginationTests;
#[cfg(all(test, windows))]
mod tests;
