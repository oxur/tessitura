use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

/// MusicBrainz API client.
#[derive(Debug, Clone)]
pub struct MusicBrainzClient {
    http: Client,
}

#[derive(Debug, Deserialize)]
pub struct MbRecording {
    pub id: String,
    pub title: String,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Option<Vec<MbArtistCredit>>,
    pub releases: Option<Vec<MbRelease>>,
    #[serde(default)]
    pub relations: Vec<MbRelation>,
}

#[derive(Debug, Deserialize)]
pub struct MbArtistCredit {
    pub artist: MbArtist,
}

#[derive(Debug, Deserialize)]
pub struct MbArtist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct MbRelease {
    pub id: String,
    pub title: String,
    #[serde(rename = "release-group")]
    pub release_group: Option<MbReleaseGroup>,
}

#[derive(Debug, Deserialize)]
pub struct MbReleaseGroup {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct MbRelation {
    #[serde(rename = "type")]
    pub relation_type: String,
    pub work: Option<MbWork>,
}

#[derive(Debug, Deserialize)]
pub struct MbWork {
    pub id: String,
    pub title: String,
}

impl MusicBrainzClient {
    /// Create a new MusicBrainz client.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new() -> Result<Self, reqwest::Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("tessitura/0.1.0 (https://github.com/oxur/tessitura)")
            .build()?;

        Ok(Self { http })
    }

    /// Get recording details by MusicBrainz ID.
    ///
    /// Rate limit: 1 request/second (enforced by caller).
    ///
    /// # Errors
    /// Returns an error if the API request fails or the response cannot be parsed.
    pub async fn get_recording(
        &self,
        mbid: &str,
    ) -> Result<MbRecording, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://musicbrainz.org/ws/2/recording/{}?inc=releases+artists+work-rels&fmt=json",
            mbid
        );

        let response = self.http.get(&url).send().await?;
        let recording = response.json::<MbRecording>().await?;

        Ok(recording)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_musicbrainz_client_creation() {
        let client = MusicBrainzClient::new();
        assert!(client.is_ok());
    }
}
