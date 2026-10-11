use super::*;
impl Provider {
    pub(super) fn search(&mut self, query: &str, page: u32) -> Result<Page<Track>, ApiError> {
        let offset = page as u64 * 50;
        let value = self.request(
            "/api/cloudsearch/pc",
            json!({"s":query.trim(),"type":1,"limit":50,"offset":offset,"total":true}),
            Crypto::Eapi,
        )?;
        let result = &value["result"];
        let count = result["songCount"]
            .as_u64()
            .ok_or_else(|| fail("SCHEMA", "网易云搜索缺少结果总数"))?;
        let items = if count == 0 && result.get("songs").is_none() {
            Vec::new()
        } else {
            metadata::tracks(&result["songs"])?
        };
        let more = offset + (items.len() as u64) < count;
        if more && items.is_empty() {
            return Err(fail("PAGINATION", "网易云搜索分页未推进"));
        }
        Ok(Page {
            items,
            page,
            has_more: more,
            cursor: None,
        })
    }
    pub(super) fn feed(&mut self) -> Result<RecommendationFeed, ApiError> {
        let authorized = self.Session.is_some();
        let mut feed = RecommendationFeed::default();
        let mut completed = 0;
        let playlistResult = if authorized {
            self.request(
                "/api/v1/discovery/recommend/resource",
                json!({}),
                Crypto::Weapi,
            )
            .and_then(|value| metadata::playlists(&value["recommend"]))
        } else {
            Err(fail("LOGIN_REQUIRED", "游客推荐不代表个人推荐"))
        };
        match playlistResult {
            Ok(items) => {
                feed.playlists = items;
                feed.playlist_source = RecommendationSource::Account;
                completed += 1;
            }
            Err(error) => {
                if authorized {
                    feed.issues.push(RecommendationIssue {
                        stage: "accountPlaylists".into(),
                        error,
                    });
                }
                match self
                    .request(
                        "/api/personalized/playlist",
                        json!({"limit":30,"total":true,"n":1000}),
                        Crypto::Weapi,
                    )
                    .and_then(|value| metadata::playlists(&value["result"]))
                {
                    Ok(items) => {
                        feed.playlists = items;
                        feed.playlist_source = RecommendationSource::Public;
                        completed += 1;
                    }
                    Err(error) => feed.issues.push(RecommendationIssue {
                        stage: "publicPlaylists".into(),
                        error,
                    }),
                }
            }
        }
        let trackResult = if authorized && self.Session.is_some() {
            self.request(
                "/api/v3/discovery/recommend/songs",
                json!({}),
                Crypto::Weapi,
            )
            .and_then(|value| metadata::tracks(&value["data"]["dailySongs"]))
        } else {
            Err(fail("LOGIN_REQUIRED", "游客新歌推荐不代表个人推荐"))
        };
        match trackResult {
            Ok(items) => {
                feed.tracks = items;
                feed.track_source = RecommendationSource::Account;
                completed += 1;
            }
            Err(error) => {
                if authorized {
                    feed.issues.push(RecommendationIssue {
                        stage: "accountTracks".into(),
                        error,
                    });
                }
                match self
                    .request(
                        "/api/personalized/newsong",
                        json!({"type":"recommend","limit":30,"areaId":0}),
                        Crypto::Weapi,
                    )
                    .and_then(|value| metadata::tracks(&value["result"]))
                {
                    Ok(items) => {
                        feed.tracks = items;
                        feed.track_source = RecommendationSource::Public;
                        completed += 1;
                    }
                    Err(error) => feed.issues.push(RecommendationIssue {
                        stage: "publicTracks".into(),
                        error,
                    }),
                }
            }
        }
        if completed == 0 {
            return Err(feed
                .issues
                .into_iter()
                .next()
                .map(|issue| issue.error)
                .unwrap_or_else(|| fail("RECOMMENDATIONS", "网易云没有返回推荐内容")));
        }
        feed.personalized = feed.playlist_source == RecommendationSource::Account
            || feed.track_source == RecommendationSource::Account;
        if feed.playlist_source == RecommendationSource::Public
            || feed.track_source == RecommendationSource::Public
        {
            feed.notice = Some(
                if authorized {
                    "部分个人推荐不可用，已明确回退为网易云公开推荐"
                } else {
                    "当前为网易云游客公开推荐，不代表本人个性推荐；播放仍以服务端实际授权为准"
                }
                .into(),
            );
        }
        Ok(feed)
    }
}
