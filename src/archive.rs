//! Interactions with the Archive API.
use anyhow::{Context, Result};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::env::var;

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveResponse {
    pub url: String,
    #[serde(rename = "FILTER")]
    pub filter: String,
}

pub async fn get_frame_record(
    frame_id: u32,
    auth_header: Option<&str>,
) -> Result<ArchiveResponse, anyhow::Error> {
    let archive_api_url =
        var("ARCHIVE_API_URL").unwrap_or(String::from("https://archive-api.lco.global"));
    let url = format!("{archive_api_url}/frames/{frame_id}/");
    let mut headers = HeaderMap::new();
    if let Some(auth_header) = auth_header {
        headers.insert("Authorization", auth_header.parse().unwrap());
    }

    reqwest::Client::new()
        .get(url)
        .headers(headers)
        .send()
        .await
        .context("Failed to send request to Archive API")?
        .json::<ArchiveResponse>()
        .await
        .context("Failed to parse JSON response from Archive")
}
