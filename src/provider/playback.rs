use super::*;
use crate::media;
impl Provider {
    pub(super) fn resolve(
        &mut self,
        track: &Track,
        quality: &str,
    ) -> Result<MediaSource, ApiError> {
        metadata::validateTrack(track)?;
        media::validateQuality(quality)?;
        let value = self.request(
            "/api/song/enhance/player/url/v1",
            json!({"ids":format!("[{}]",track.id),"level":quality,"encodeType":"flac"}),
            Crypto::Xeapi,
        )?;
        media::playback(&value, &track.id, quality).map(|(source, _)| source)
    }
    pub(super) fn qualities(&mut self, track: &Track) -> Result<QualityList, ApiError> {
        let mut list = media::catalogue();
        for (index, (quality, _)) in media::levels.iter().enumerate() {
            let value = self.request(
                "/api/song/enhance/player/url/v1",
                json!({"ids":format!("[{}]",track.id),"level":quality,"encodeType":"flac"}),
                Crypto::Xeapi,
            )?;
            let result = media::playback(&value, &track.id, quality).map(|(_, size)| size);
            if let Err(error) = &result
                && !matches!(
                    error.code.as_str(),
                    "PLAYBACK_DENIED"
                        | "TRIAL_ONLY"
                        | "QUALITY_DENIED"
                        | "PERMISSION_DENIED"
                        | "REMOTE"
                )
            {
                return Err(error.clone());
            }
            list.Items.push(media::qualityItem(index, result));
        }
        list.validate()?;
        Ok(list)
    }
}
