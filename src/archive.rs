//! Interactions with the Archive API.
use crate::multipart::MultipartDownloader;
use anyhow::{Context, Result};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::env::var;
use tracing::{debug, info};

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

/// Download frame data using multipart downloader for large files
pub async fn download_frame_data(url: &str) -> Result<Vec<u8>, anyhow::Error> {
    debug!("Starting frame data download from URL: {}", url);
    let downloader = MultipartDownloader::new();
    match downloader.download(url).await {
        Ok(data) => Ok(data),
        Err(e) => {
            debug!(
                "Multipart download failed: {}, attempting fallback to simple reqwest",
                e
            );

            // Fallback to simple reqwest download for S3 URLs that don't support multipart
            info!("Falling back to single-connection download for S3 presigned URL");
            let response = reqwest::get(url)
                .await
                .context("Failed to download with fallback reqwest method")?;

            let bytes = response
                .bytes()
                .await
                .context("Failed to read response bytes with fallback method")?;

            info!(
                "Fallback download successful: {}MB",
                bytes.len() / (1024 * 1024)
            );

            Ok(bytes.to_vec())
        }
    }
}
