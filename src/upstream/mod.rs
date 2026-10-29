//! Client for the OpenTripPlanner GraphQL API behind muoversiatorino.it.

use std::time::Duration;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;

mod queries;
pub mod types;

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("upstream timed out")]
    Timeout,
    #[error("upstream unreachable: {0}")]
    Unreachable(String),
    #[error("upstream returned HTTP {0}")]
    Status(u16),
    /// Usually means the upstream schema changed and our queries need fixing.
    #[error("upstream GraphQL error: {0}")]
    GraphQl(String),
    #[error("invalid upstream response: {0}")]
    InvalidResponse(String),
}

impl UpstreamError {
    fn from_reqwest(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else {
            Self::Unreachable(err.to_string())
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpstreamClient {
    http: reqwest::Client,
    url: reqwest::Url,
}

#[derive(Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

impl UpstreamClient {
    pub fn new(url: reqwest::Url, timeout: Duration) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self { http, url })
    }

    /// Stop details and departures in the next `time_range`. `None` when the stop does not exist.
    pub async fn stop_departures(
        &self,
        stop_id: &str,
        time_range: Duration,
        max_departures: u32,
    ) -> Result<Option<types::Stop>, UpstreamError> {
        let data: types::StopData = self
            .query(
                queries::STOP_DEPARTURES,
                json!({
                    "id": stop_id,
                    "timeRange": time_range.as_secs(),
                    "numberOfDepartures": max_departures,
                }),
            )
            .await?;
        Ok(data.stop)
    }

    async fn query<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T, UpstreamError> {
        let response = self
            .http
            .post(self.url.clone())
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(UpstreamError::from_reqwest)?;
        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(UpstreamError::from_reqwest)?;

        // GraphQL errors come with HTTP 500, so look at the body before the status.
        match serde_json::from_slice::<GraphQlResponse<T>>(&body) {
            Ok(response) if !response.errors.is_empty() => Err(UpstreamError::GraphQl(
                response
                    .errors
                    .into_iter()
                    .map(|err| err.message)
                    .collect::<Vec<_>>()
                    .join("; "),
            )),
            _ if !status.is_success() => Err(UpstreamError::Status(status.as_u16())),
            Ok(GraphQlResponse {
                data: Some(data), ..
            }) => Ok(data),
            Ok(_) => Err(UpstreamError::InvalidResponse("missing data".to_owned())),
            Err(err) => Err(UpstreamError::InvalidResponse(err.to_string())),
        }
    }
}
