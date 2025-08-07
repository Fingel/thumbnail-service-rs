use anyhow::Result;
use axum::{
    Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
};
use serde_json::{Value, json};
use tower_http::{self, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod archive;

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
    let archive_response = archive::get_frame_record(frame_id, auth_header).await?;
    Ok(Json(
        json!({"url": archive_response.url, "filter": archive_response.filter}),
    ))
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
