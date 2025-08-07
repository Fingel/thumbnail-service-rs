use axum::{Router, http::StatusCode, response::Json, routing::get};
use serde_json::{Value, json};
use tower_http::{self, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

async fn hello() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({"data": "Hello, World!"})))
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

    let app = Router::new().route("/", get(hello).layer(TraceLayer::new_for_http()));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
