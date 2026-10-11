use super::*;
use std::collections::{BTreeMap, HashSet};

const pageSize: usize = 50;
impl Provider {
    pub(super) fn library(&mut self) -> Result<Vec<Playlist>, ApiError> {
        let user = self
            .Session
            .as_ref()
            .ok_or_else(|| fail("LOGIN_REQUIRED", "网易云个人歌单需要本人扫码授权"))?
            .UserId
            .clone();
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for page in 0..200 {
            let value = self.request(
                "/api/user/playlist",
                json!({"uid":user,"limit":100,"offset":page * 100,"includeVideo":true}),
                Crypto::Weapi,
            )?;
            let list = metadata::playlists(&value["playlist"])?;
            let count = list.len();
            let more = value["more"]
                .as_bool()
                .ok_or_else(|| fail("SCHEMA", "网易云歌单目录缺少分页状态"))?;
            if more && count == 0 {
                return Err(fail("PAGINATION", "网易云歌单目录没有推进分页"));
            }
            for item in list {
                if !seen.insert(item.id.clone()) {
                    return Err(fail("PAGINATION", "网易云歌单目录重复，拒绝返回不完整结果"));
                }
                result.push(item);
            }
            if !more {
                return Ok(result);
            }
        }
        Err(fail(
            "RESPONSE_LIMIT",
            "网易云歌单目录超过安全读取限制，未返回截断列表",
        ))
    }
    pub(super) fn playlist(
        &mut self,
        id: &str,
        page: u32,
        cursor: Option<&str>,
    ) -> Result<Page<Track>, ApiError> {
        if self.Session.is_some() {
            self.checkSession()?;
        }
        let revision = self.revision()?.to_owned();
        let offset = page as usize * pageSize;
        if let Some(cursor) = cursor {
            let snapshot = self.Playlists.get(id).ok_or_else(|| {
                fail("CURSOR_EXPIRED", "网易云歌单分页快照已失效，请重新打开歌单")
            })?;
            let expected = format!("{}:{offset}", snapshot.Key);
            if cursor != expected
                || snapshot.Revision != revision
                || snapshot.Expires <= Instant::now()
                || snapshot.Id != id
            {
                return Err(fail(
                    "CURSOR_EXPIRED",
                    "网易云歌单游标、页码或授权代次不匹配",
                ));
            }
        } else {
            let value = self.request(
                "/api/v6/playlist/detail",
                json!({"id":id,"n":100000,"s":0}),
                Crypto::Eapi,
            )?;
            let playlist = &value["playlist"];
            if metadata::id(&playlist["id"])? != id {
                return Err(fail("SCHEMA", "网易云歌单详情标识不匹配"));
            }
            let ids = playlist["trackIds"]
                .as_array()
                .ok_or_else(|| fail("SCHEMA", "网易云歌单详情缺少完整歌曲标识列表"))?;
            if ids.len() > 100000
                || playlist["trackCount"]
                    .as_u64()
                    .is_some_and(|count| count > ids.len() as u64)
            {
                return Err(fail(
                    "PLAYLIST_INCOMPLETE",
                    "网易云未授权或未返回完整歌单，不能伪称分页完成",
                ));
            }
            let tracks = ids
                .iter()
                .map(|item| metadata::id(&item["id"]))
                .collect::<Result<Vec<_>, _>>()?;
            if self.Playlists.len() >= 8 {
                self.Playlists.clear();
            }
            self.Playlists.insert(
                id.into(),
                PlaylistSnapshot {
                    Id: id.into(),
                    Key: uuid::Uuid::new_v4().to_string(),
                    Revision: self.revision()?.into(),
                    Tracks: tracks,
                    Expires: Instant::now() + Duration::from_secs(600),
                },
            );
        }
        let snapshot = self.Playlists.get(id).unwrap();
        if offset > snapshot.Tracks.len() {
            return Err(fail("PARAMETERS", "网易云歌单页码超出范围"));
        }
        let end = (offset + pageSize).min(snapshot.Tracks.len());
        let ids = snapshot.Tracks[offset..end].to_vec();
        let more = end < snapshot.Tracks.len();
        let next = more.then(|| format!("{}:{end}", snapshot.Key));
        if ids.is_empty() {
            return Ok(Page {
                items: Vec::new(),
                page,
                has_more: false,
                cursor: None,
            });
        }
        let c: Vec<_> = ids
            .iter()
            .map(|id| json!({"id":id.parse::<u64>().unwrap()}))
            .collect();
        let value = self.request("/api/v3/song/detail", json!({"c":serde_json::to_string(&c).map_err(|_| fail("SCHEMA", "网易云歌曲参数编码失败"))?}), Crypto::Weapi)?;
        let songs = metadata::tracks(&value["songs"])?;
        let songs: BTreeMap<_, _> = songs
            .into_iter()
            .map(|song| (song.id.clone(), song))
            .collect();
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            items.push(songs.get(&id).cloned().ok_or_else(|| {
                fail(
                    "PLAYLIST_INCOMPLETE",
                    "网易云未返回所请求歌曲详情，不能跳过歌曲伪称完整",
                )
            })?);
        }
        Ok(Page {
            items,
            page,
            has_more: more,
            cursor: next,
        })
    }
}
