mod auth;
mod health;

use axum::Router;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let app = build_router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind gateway listener");

    tracing::info!(addr = %listener.local_addr().unwrap(), "gateway listening");

    axum::serve(listener, app)
        .await
        .expect("gateway server error");
}

fn build_router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(health::health))
        .nest("/auth", auth::router())
}
