use anyhow::{Context, Error, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client as S3Client, presigning::PresigningConfig, primitives::ByteStream};
use std::{env::var, time::Duration};
use tracing::debug;

pub struct S3Service {
    client: S3Client,
    bucket_name: String,
}

impl S3Service {
    pub async fn new() -> Self {
        let bucket_name = var("AWS_BUCKET_NAME").expect("AWS_BUCKET_NAME not set");
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let s3 = aws_sdk_s3::Client::new(&config);
        Self {
            client: s3,
            bucket_name,
        }
    }

    pub async fn upload_file(&self, body: &[u8], key: &str) -> Result<String, Error> {
        let bstream = ByteStream::from(body.to_vec());
        let put_object = self
            .client
            .put_object()
            .bucket(&self.bucket_name)
            .key(key)
            .body(bstream)
            .content_type("image/jpeg")
            .send()
            .await
            .context("Failed to upload file to s3")?;
        debug!("Uploaded file to S3, ETag:  {:?}", put_object.e_tag());
        Ok(format!("s3://{}/{}", self.bucket_name, key))
    }

    pub async fn presigned_url(&self, key: &str) -> Result<String, Error> {
        let expires_in = Duration::from_secs(60 * 60);
        let presign_config = PresigningConfig::expires_in(expires_in)
            .context("Failed to create presigning config")?;
        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(key)
            .presigned(presign_config)
            .await
            .context("Failed to generate presigned URL")?;
        Ok(presigned_request.uri().to_string())
    }

    pub async fn file_exists(&self, key: &str) -> bool {
        self.client
            .head_object()
            .bucket(&self.bucket_name)
            .key(key)
            .send()
            .await
            .is_ok()
    }
}
