use anyhow::Result;
use axum::{
    Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
};
use image::DynamicImage;
use image::ImageBuffer;
use serde_json::{Value, json};
use std::time::Instant;
use std::{env::var, io::Cursor};
use tower_http::{self, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod archive;
mod fits;
mod s3;
mod scaling;

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {:#}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

async fn hello() -> String {
    "Welcome to the Thumbnail Service!".to_string()
}

async fn thumbnail(Path(frame_id): Path<u32>, headers: HeaderMap) -> Result<Json<Value>, AppError> {
    let s3_service = s3::S3Service::new().await;
    let auth_header: Option<&str> = headers
        .get("Authorization")
        .map(|v| v.to_str().unwrap_or_default());
    let frame_record = archive::get_frame_record(frame_id, auth_header).await?;
    let key = format!("frames/{frame_id}.jpeg");
    if cache_disabled() || !s3_service.file_exists(&key).await {
        tracing::debug!("Starting download of frame {frame_id}");
        let frame_bytes = reqwest::get(frame_record.url).await?.bytes().await?;
        tracing::debug!(
            "Done downloading frame {frame_id} size: {}mb",
            frame_bytes.len() / (1024 * 1024)
        );
        tracing::debug!("Starting open fits");
        let cursor = Cursor::new(&frame_bytes[..]);
        let image_data = fits::read_fits(cursor).unwrap();
        tracing::debug!("Done open fits");
        let now = Instant::now();
        let scaled_image = scaling::scaled_image(image_data.pixels);
        let elapsed = now.elapsed();
        tracing::debug!("Scaling took {:?}", elapsed);
        let mut image = DynamicImage::ImageLuma8(
            ImageBuffer::from_vec(image_data.width, image_data.height, scaled_image).unwrap(),
        );
        image = image.resize(300, 300, image::imageops::FilterType::Triangle);
        let mut image_buf = Vec::new();
        let mut writer = Cursor::new(&mut image_buf);
        image.write_to(&mut writer, image::ImageFormat::Jpeg)?;
        s3_service.upload_file(&image_buf, &key).await?;
    }
    let url = s3_service.presigned_url(&key).await?;
    Ok(Json(json!({"url": url})))
}

fn cache_disabled() -> bool {
    match var("USE_S3_CACHE") {
        Ok(value) => value == "false",
        Err(_) => false,
    }
}

#[tokio::main]
async fn main() {
    // Tracing AKA logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(hello))
        .route("/{frame_id}/", get(thumbnail))
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
