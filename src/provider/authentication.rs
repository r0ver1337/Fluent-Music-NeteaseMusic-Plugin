use super::*;
impl Provider {
    pub(super) fn account(&mut self) -> Result<Account, ApiError> {
        let user = self
            .Session
            .as_ref()
            .ok_or_else(|| fail("LOGIN_REQUIRED", "请先使用网易云手机应用扫码授权"))?
            .UserId
            .clone();
        let value = self.request("/api/nuser/account/get", json!({}), Crypto::Weapi)?;
        let (id, account) = metadata::account(&value)?;
        if id != user {
            return Err(fail("ACCOUNT_MISMATCH", "网易云返回的账号与授权记录不匹配"));
        }
        Ok(account)
    }
    pub(super) fn loginBegin(&mut self) -> Result<ResultData, ApiError> {
        self.Pending = None;
        let mut cookies = self.Guest.clone();
        self.client().guest(&mut cookies)?;
        if cookies != self.Guest {
            self.Store
                .saveGuest(&self.Client.as_ref().unwrap().Device, &cookies)?;
        }
        self.Guest = cookies.clone();
        let value = self.client().request(
            "/api/login/qrcode/unikey",
            json!({"type":3}),
            Crypto::Eapi,
            &mut cookies,
            &[200],
        )?;
        let key = value["unikey"]
            .as_str()
            .filter(|key| {
                !key.is_empty()
                    && key.len() <= 2048
                    && key
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            })
            .ok_or_else(|| fail("LOGIN", "网易云未返回有效二维码密钥"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut url = url::Url::parse("https://music.163.com/login").unwrap();
        url.query_pairs_mut().append_pair("codekey", key);
        self.Pending = Some(Pending {
            Id: id.clone(),
            Key: Zeroizing::new(key.into()),
            Cookies: cookies,
            Revision: self.revision()?.into(),
            Expires: Instant::now() + Duration::from_secs(180),
            NextPoll: Instant::now(),
            State: LoginState::Waiting,
        });
        Ok(ResultData::LoginChallenge(LoginChallenge {
            challenge_id: id,
            qr_text: url.into(),
            expires_in_seconds: 180,
        }))
    }
    pub(super) fn loginPoll(
        &mut self,
        id: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ResultData, ApiError> {
        let pending = self
            .Pending
            .as_ref()
            .ok_or_else(|| fail("LOGIN", "没有待确认的网易云二维码"))?;
        if pending.Id != id {
            return Err(fail("LOGIN", "网易云二维码任务不匹配或已失效"));
        }
        if Instant::now() >= pending.Expires {
            self.Pending = None;
            return Ok(ResultData::LoginState(LoginState::Expired));
        }
        if Instant::now() < pending.NextPoll {
            return Ok(ResultData::LoginState(pending.State.clone()));
        }
        let key = Zeroizing::new(pending.Key.to_string());
        let mut cookies = pending.Cookies.clone();
        let revision = pending.Revision.clone();
        self.Store
            .check(&self.Client.as_ref().unwrap().Device, &revision)?;
        let value = self.client().request(
            "/api/login/qrcode/client/login",
            json!({"key":key.as_str(),"type":3}),
            Crypto::Eapi,
            &mut cookies,
            &[800, 801, 802, 803],
        )?;
        if let Some(pending) = self.Pending.as_mut() {
            pending.Cookies = cookies.clone();
            pending.NextPoll = Instant::now() + Duration::from_secs(2);
        }
        let state = match value["code"].as_u64() {
            Some(800) => {
                self.Pending = None;
                LoginState::Expired
            }
            Some(801) => LoginState::Waiting,
            Some(802) => LoginState::Scanned,
            Some(803) => {
                if !cookies.authorized() {
                    return Err(fail("LOGIN", "网易云授权成功响应缺少本人凭据"));
                }
                let profile = self.client().request(
                    "/api/nuser/account/get",
                    json!({}),
                    Crypto::Weapi,
                    &mut cookies,
                    &[200],
                )?;
                let (userId, account) = metadata::account(&profile)?;
                let session = Session {
                    UserId: userId,
                    Cookies: cookies,
                };
                self.client().ensureTime()?;
                if cancelled() {
                    return Err(fail("CANCELLED", "网易云请求已取消，未保存授权"));
                }
                let next =
                    self.Store
                        .save(&self.Client.as_ref().unwrap().Device, &session, &revision)?;
                self.Session = Some(session);
                self.Revision = Some(next);
                self.Pending = None;
                self.Playlists.clear();
                self.client().resetTransport();
                LoginState::Authorized(account)
            }
            _ => return Err(fail("LOGIN", "网易云二维码状态无法识别")),
        };
        if let Some(pending) = self.Pending.as_mut() {
            pending.State = state.clone();
        }
        Ok(ResultData::LoginState(state))
    }
}
