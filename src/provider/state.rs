use super::*;
impl Provider {
    pub(super) fn ensureClient(&mut self) -> Result<(), ApiError> {
        if self.Client.is_none() {
            self.Client = Some(Client::new(self.Store.device()?)?);
        }
        Ok(())
    }
    pub(super) fn client(&mut self) -> &mut Client {
        self.Client.as_mut().expect("client initialized")
    }
    pub(super) fn revision(&self) -> Result<&str, ApiError> {
        self.Revision
            .as_deref()
            .ok_or_else(|| fail("SESSION_STORAGE", "网易云授权代次缺失"))
    }
    pub(super) fn restore(&mut self) -> Result<(), ApiError> {
        if !self.Restored {
            let snapshot = self.Store.snapshot(&self.Client.as_ref().unwrap().Device)?;
            self.Revision = Some(snapshot.Revision);
            self.Session = snapshot.Session;
            if !self.Guest.guest() {
                self.Guest = self.Store.guest(&self.Client.as_ref().unwrap().Device)?;
            }
            self.Restored = true;
        }
        Ok(())
    }
    pub(super) fn checkSession(&mut self) -> Result<(), ApiError> {
        let result = self
            .Store
            .check(&self.Client.as_ref().unwrap().Device, self.revision()?);
        if result.is_err() {
            self.Session = None;
            self.Pending = None;
            self.Playlists.clear();
        }
        result
    }
    pub(super) fn request(
        &mut self,
        uri: &str,
        data: Value,
        kind: Crypto,
    ) -> Result<Value, ApiError> {
        self.client().ensureTime()?;
        if self.Session.is_some() {
            self.checkSession()?;
            let mut session = self.Session.as_ref().unwrap().clone();
            let value = self
                .client()
                .request(uri, data, kind, &mut session.Cookies, &[200]);
            if let Err(error) = &value
                && error.code == "LOGIN_REQUIRED"
            {
                self.Session = None;
                self.Pending = None;
                self.Playlists.clear();
                self.Revision = Some(
                    self.Store
                        .clearExpected(&self.Client.as_ref().unwrap().Device, self.revision()?)?,
                );
                self.client().resetTransport();
            }
            let value = value?;
            if session.Cookies != self.Session.as_ref().unwrap().Cookies {
                if session
                    .Cookies
                    .sameAuthorization(&self.Session.as_ref().unwrap().Cookies)
                {
                    self.Store.update(
                        &self.Client.as_ref().unwrap().Device,
                        &session,
                        self.revision()?,
                    )?;
                } else {
                    let revision = self.Store.save(
                        &self.Client.as_ref().unwrap().Device,
                        &session,
                        self.revision()?,
                    )?;
                    self.Revision = Some(revision);
                    self.Playlists.clear();
                    self.Pending = None;
                }
                self.Session = Some(session);
            }
            Ok(value)
        } else {
            let mut cookies = self.Guest.clone();
            self.client().guest(&mut cookies)?;
            if cookies != self.Guest {
                self.Store
                    .saveGuest(&self.Client.as_ref().unwrap().Device, &cookies)?;
                self.Guest = cookies.clone();
            }
            let value = self
                .client()
                .request(uri, data, kind, &mut cookies, &[200])?;
            if cookies != self.Guest {
                self.Store
                    .saveGuest(&self.Client.as_ref().unwrap().Device, &cookies)?;
            }
            self.Guest = cookies;
            Ok(value)
        }
    }
}
