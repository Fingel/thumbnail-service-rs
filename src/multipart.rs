use anyhow::{Context, Result};
use reqwest::{Client, header::RANGE};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct MultipartConfig {
    /// Size of each chunk in bytes (default: 16MB)
    pub chunk_size: usize,
    /// Maximum number of concurrent downloads (default: 4)
    pub max_concurrent: usize,
    /// Minimum file size to trigger multipart download (default: 100MB)
    pub min_file_size: usize,
}

impl Default for MultipartConfig {
    fn default() -> Self {
        Self {
            chunk_size: 16 * 1024 * 1024, // 16MB
            max_concurrent: 4,
            min_file_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Represents a chunk of data to be downloaded
#[derive(Debug, Clone)]
struct DownloadChunk {
    start: usize,
    end: usize,
    index: usize,
}

pub struct MultipartDownloader {
    client: Client,
    config: MultipartConfig,
}

impl MultipartDownloader {
    pub fn new() -> Self {
        Self::with_config(MultipartConfig::default())
    }

    pub fn with_config(config: MultipartConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Download a file, using multipart if it's large enough
    pub async fn download(&self, url: &str) -> Result<Vec<u8>> {
        match self.test_range_support_and_get_size(url).await {
            Ok(file_size) => {
                if file_size < self.config.min_file_size {
                    debug!(
                        "File size {}MB is below {}MB threshold, using single download",
                        file_size / (1024 * 1024),
                        self.config.min_file_size / (1024 * 1024)
                    );
                    return self.download_single(url).await;
                }

                debug!(
                    "File size {}MB exceeds {}MB threshold, using multipart download with {} concurrent connections",
                    file_size / (1024 * 1024),
                    self.config.min_file_size / (1024 * 1024),
                    self.config.max_concurrent
                );
                self.download_multipart(url, file_size).await
            }
            Err(e) => {
                debug!(
                    "Failed to get file size ({}), falling back to single download",
                    e
                );
                self.download_single(url).await
            }
        }
    }

    /// Test range request support and attempt to determine file size
    async fn test_range_support_and_get_size(&self, url: &str) -> Result<usize> {
        // Try to download first 1KB to test range support
        let response = self
            .client
            .get(url)
            .header(RANGE, "bytes=0-1023")
            .send()
            .await
            .context("Failed to send test range request")?;

        if response.status().as_u16() == 206 {
            // Server supports range requests (206 Partial Content)
            debug!("Server supports range requests (got 206 response)");

            // Try to get file size from content-range header
            if let Some(content_range) = response.headers().get("content-range") {
                if let Ok(range_str) = content_range.to_str() {
                    // Content-Range: bytes 0-1023/total_size
                    if let Some(total_size_str) = range_str.split('/').nth(1) {
                        if let Ok(total_size) = total_size_str.parse::<usize>() {
                            debug!(
                                "File size from content-range: {}MB",
                                total_size / (1024 * 1024)
                            );
                            return Ok(total_size);
                        }
                    }
                }
            }
        }

        // If we can't determine size or range support, return error to fall back to single download
        Err(anyhow::anyhow!(
            "Cannot determine file size or range request support for multipart download"
        ))
    }

    /// Download file using a single request
    async fn download_single(&self, url: &str) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();
        debug!("Starting single request download");
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send GET request")?;
        let bytes = response
            .bytes()
            .await
            .context("Failed to read response body")?;
        let elapsed = start_time.elapsed();
        debug!("Single download completed in : {:?}", elapsed);

        Ok(bytes.to_vec())
    }

    /// Download file using multiple parallel requests
    async fn download_multipart(&self, url: &str, file_size: usize) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();
        let chunks = self.calculate_chunks(file_size);
        let num_chunks = chunks.len();

        debug!(
            "Starting multipart download: {} chunks of up to {}MB each with {} max concurrent connections",
            num_chunks,
            self.config.chunk_size / (1024 * 1024),
            self.config.max_concurrent
        );

        // Create semaphore to limit concurrent downloads
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));

        // Store results with their original index
        let mut tasks = Vec::new();

        for chunk in chunks {
            let client = self.client.clone();
            let url = url.to_string();
            let semaphore = semaphore.clone();

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                Self::download_chunk(&client, &url, chunk).await
            });

            tasks.push(task);
        }

        // Wait for all downloads to complete
        let mut results = Vec::with_capacity(num_chunks);
        for task in tasks {
            let result = task
                .await
                .context("Download task panicked")?
                .context("Failed to download chunk")?;
            results.push(result);
        }

        // Sort results by chunk index to maintain order
        results.sort_by_key(|(_, index)| *index);

        // Combine all chunks into final result
        let total_size: usize = results.iter().map(|(data, _)| data.len()).sum();
        let mut final_data = Vec::with_capacity(total_size);

        for (chunk_data, _) in results {
            final_data.extend_from_slice(&chunk_data);
        }
        let elapsed = start_time.elapsed();

        debug!("Multipart download completed: in {:?}", elapsed);
        Ok(final_data)
    }

    /// Calculate chunk ranges for parallel download
    fn calculate_chunks(&self, file_size: usize) -> Vec<DownloadChunk> {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut index = 0;

        while start < file_size {
            let end = std::cmp::min(start + self.config.chunk_size - 1, file_size - 1);
            chunks.push(DownloadChunk { start, end, index });
            start = end + 1;
            index += 1;
        }

        chunks
    }

    /// Download a specific chunk of data
    async fn download_chunk(
        client: &Client,
        url: &str,
        chunk: DownloadChunk,
    ) -> Result<(Vec<u8>, usize)> {
        let start_time = std::time::Instant::now();
        let range_header = format!("bytes={}-{}", chunk.start, chunk.end);

        debug!(
            "Starting chunk {} download (bytes {}-{})",
            chunk.index, chunk.start, chunk.end
        );

        let response = client
            .get(url)
            .header(RANGE, range_header)
            .send()
            .await
            .context("Failed to send range request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Range request failed with status: {}",
                response.status()
            ));
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read chunk response body")?;

        let elapsed = start_time.elapsed();
        debug!(
            "Chunk {} completed: {} bytes in {:?}",
            chunk.index,
            bytes.len(),
            elapsed
        );
        Ok((bytes.to_vec(), chunk.index))
    }
}

impl Default for MultipartDownloader {
    fn default() -> Self {
        Self::new()
    }
}
