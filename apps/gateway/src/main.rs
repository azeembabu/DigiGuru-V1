mod admin;
mod audit;
mod auth;
mod extractors;
mod health;
mod me;
mod reference;
mod state;
mod student;
mod validation;

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::{header, header::HeaderName, HeaderValue, Method};
use axum::Router;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use dg_core::config::Config;
use state::AppState;

#[tokio::main]
async fn main() {
    // Non-fatal: in production, real secrets come from the environment
    // (or a secrets manager) directly, never from a committed `.env`
    // (`.claude/rules/security.md`). Local/dev loads `.env` if present.
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env().unwrap_or_else(|err| {
        // A missing required env var is a startup-abort condition, per
        // `.claude/rules/code-style.md` ("panic! ... allowed ... in `main`
        // during startup where a failure should abort the process").
        eprintln!("configuration error: {err}");
        std::process::exit(1);
    });

    let pool = dg_db::create_pool(&config.database_url)
        .await
        .expect("failed to connect to Postgres");

    let redis_client = redis::Client::open(config.redis_url.clone()).expect("invalid REDIS_URL");
    let redis = redis::aio::ConnectionManager::new(redis_client)
        .await
        .expect("failed to connect to Redis");

    let port = config.port;
    let state = AppState::new(pool, redis, config);

    let app = build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind gateway listener");

    tracing::info!(addr = %addr, "gateway listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("gateway server error");
}

fn build_router(state: AppState) -> Router {
    let max_upload_bytes = state.config.max_upload_bytes;

    let api_v1 = Router::new()
        .nest("/auth", auth::router())
        .nest("/reference", reference::router())
        .nest("/me", me::router())
        .nest("/student", student::router())
        .nest("/admin", admin::router(max_upload_bytes));

    Router::new()
        .route("/health", axum::routing::get(health::health))
        .nest("/api/v1", api_v1)
        .layer(TraceLayer::new_for_http())
        // CORS: an explicit origin allow-list, never `Any`. `apps/web` now
        // exists and calls this from its own origin, so that origin has to be
        // named here — and because cookies are the auth transport
        // (`.claude/rules/security.md`), `allow_credentials(true)` makes a
        // wildcard origin both invalid per the CORS spec and unsafe. Origins
        // come from `WEB_ORIGIN` (comma-separated) so deployments set their
        // own; the default is the local Next.js dev server.
        .layer(cors_layer())
        .with_state(state)
}

/// Build the CORS layer from `WEB_ORIGIN` (comma-separated), defaulting to
/// the local Next.js dev server. An origin that will not parse is skipped
/// with a warning rather than aborting startup.
fn cors_layer() -> CorsLayer {
    let raw = std::env::var("WEB_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let origins: Vec<HeaderValue> = raw
        .split(',')
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .filter_map(|o| match o.parse::<HeaderValue>() {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin = %o, "ignoring unparseable WEB_ORIGIN entry");
                None
            }
        })
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE])
        // Paginated admin lists report their pre-pagination total in
        // `X-Total-Count` (`.claude/rules/api-conventions.md`). A response
        // header is invisible to a browser client unless it is exposed, so
        // without this the header may as well not be sent.
        .expose_headers([HeaderName::from_static("x-total-count")])
}

/// Graceful shutdown on SIGINT (Ctrl+C) or SIGTERM (container/orchestrator
/// stop). A live audio/WS gateway must drain, not hard-kill, connections —
/// this scaffold wires the signal; the actual drain logic belongs to the
/// Phase 3 WS layer once it exists.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl+C, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }

    // Give in-flight requests a moment to complete before axum's own
    // graceful-shutdown drain proceeds.
    tokio::time::sleep(Duration::from_millis(50)).await;
}
