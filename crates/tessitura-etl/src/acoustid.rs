use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::enrich::resilience::RateLimiter;

/// AcoustID API client.
#[derive(Debug, Clone)]
pub struct AcoustIdClient {
    http: Client,
    api_key: String,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdResponse {
    pub status: String,
    pub results: Vec<AcoustIdResult>,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdResult {
    pub id: String,
    pub score: f64,
    pub recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdRecording {
    pub id: String, // MusicBrainz recording ID
    pub title: Option<String>,
    pub artists: Option<Vec<AcoustIdArtist>>,
    pub releases: Option<Vec<AcoustIdRelease>>,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdArtist {
    pub id: String, // MusicBrainz artist ID
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdRelease {
    pub id: String, // MusicBrainz release ID
    pub title: Option<String>,
}

impl AcoustIdClient {
    /// Create a new AcoustID client with 3 req/sec rate limiting.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(api_key: impl Into<String>) -> Result<Self, reqwest::Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("tessitura/0.1.0 (https://github.com/oxur/tessitura)")
            .build()?;

        Ok(Self {
            http,
            api_key: api_key.into(),
            rate_limiter: RateLimiter::new(3), // 3 req/sec
        })
    }

    /// Look up a fingerprint via AcoustID API (rate-limited at 3 req/sec).
    ///
    /// # Errors
    /// Returns an error if the API request fails or the response cannot be parsed.
    pub async fn lookup(
        &self,
        fingerprint: &str,
        duration: f64,
    ) -> Result<AcoustIdResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Rate limit: 3 requests per second
        self.rate_limiter.acquire().await;

        let url = "https://api.acoustid.org/v2/lookup";

        let response = self
            .http
            .post(url)
            .form(&[
                ("client", self.api_key.as_str()),
                ("fingerprint", fingerprint),
                ("duration", &duration.to_string()),
                ("meta", "recordings releases releasegroups"),
            ])
            .send()
            .await?;

        let result = response.json::<AcoustIdResponse>().await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acoustid_client_creation() {
        let client = AcoustIdClient::new("test-key");
        assert!(client.is_ok());
    }
}
