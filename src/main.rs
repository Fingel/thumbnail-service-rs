use anyhow::Result;
use axum::{
    Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
};
use serde_json::{Value, json};
use std::io::Cursor;
use tower_http::{self, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod archive;
mod fits;

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
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
    let auth_header: Option<&str> = headers
        .get("Authorization")
        .map(|v| v.to_str().unwrap_or_default());
    let frame_record = archive::get_frame_record(frame_id, auth_header).await?;
    tracing::debug!("Starting download of frame {frame_id}");
    let frame_bytes = reqwest::get(frame_record.url).await?.bytes().await?;
    tracing::debug!("Done downloading frame {frame_id}");
    let cursor = Cursor::new(frame_bytes.to_vec());
    let image_data = fits::read_fits(cursor).unwrap();
    tracing::debug!(
        "Image width: {}, height: {}, pixels: {}",
        image_data.width,
        image_data.height,
        image_data.pixels.len()
    );
    let frame_size = frame_bytes.len();
    Ok(Json(json!({"frame_size_mb": frame_size / (1024 * 1024)})))
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
