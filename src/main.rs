mod account;
mod config;
mod error;
mod mcc_data;
mod tracing_config;
pub mod transaction;
mod utils;
mod ws_handler;

use crate::account::handler::{
    add_account, delete_account, get_account_monitor, get_accounts, get_accounts_monitors,
    update_account, update_accounts_stat,
};
use crate::mcc_data::get_all_mcc;
use crate::transaction::handler::{
    download_csv, download_json, download_xlsx, get_mcc, get_transactions,
};
use crate::ws_handler::{WsTx, handle_socket};
use axum::Router;
use axum::extract::FromRef;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{any, delete, get, get_service, post, put};
use sqlx::SqlitePool;
use std::fs::File;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{debug, info};
use tracing_log::LogTracer;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub ws_tx: WsTx,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for WsTx {
    fn from_ref(state: &AppState) -> Self {
        state.ws_tx.clone()
    }
}

#[tokio::main]
async fn main() {
    // initialize tracing with environment-controlled level
    LogTracer::init().expect("Failed to set logger");
    tracing_config::init();

    info!("Starting transactions-service");

    if std::path::Path::new("bills.db").exists() {
        std::fs::rename("bills.db", "mimir.db").expect("Failed to rename bills.db to mimir.db");
        info!("Renamed bills.db to mimir.db");
    }
    let _ = File::create_new("mimir.db");

    debug!("Connecting to SQLite database at mimir.db");
    let pool = SqlitePool::connect("mimir.db").await.unwrap();

    debug!("Running database migrations");
    sqlx::migrate!().run(&pool).await.unwrap();
    info!("Database connected and migrations applied");

    let (ws_tx, _) = tokio::sync::broadcast::channel::<String>(64);

    // Serve Angular app from the dist folder, falling back to index.html for SPA routing
    let serve_dir =
        ServeDir::new("./static").not_found_service(ServeFile::new("./static/index.html"));

    let app = Router::new()
        // Add API routes here, e.g.:
        .route("/mimir/api/mcc", get(get_all_mcc))
        .route("/mimir/api/accounts", get(get_accounts).post(add_account))
        .route("/mimir/api/accounts/stat", put(update_accounts_stat))
        .route(
            "/mimir/api/accounts/{ida}",
            put(update_account).delete(delete_account),
        )
        .route("/mimir/api/transactions", post(get_transactions))
        .route("/mimir/api/transactions/csv", post(download_csv))
        .route("/mimir/api/transactions/xlsx", post(download_xlsx))
        .route("/mimir/api/transactions/json", post(download_json))
        // .route(
        //     "/mimir/api/accounts/{ida}/transactions",
        //     get(get_transactions_by_ida),
        // )
        .route("/mimir/api/monitors", get(get_accounts_monitors))
        .route(
            "/mimir/api/monitors/{external_id}",
            get(get_account_monitor),
        )
        .route("/mimir/ws", any(handle_socket))
        // Serve the Angular SPA under
        .nest_service("/transactions", serve_dir.clone())
        .fallback_service(get_service(serve_dir.clone()))
        .layer(
            CorsLayer::new()
                .allow_origin("http://localhost:4200".parse::<HeaderValue>().unwrap())
                .allow_methods([
                    Method::OPTIONS,
                    Method::GET,
                    Method::DELETE,
                    Method::POST,
                    Method::PUT,
                ])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]),
        )
        .with_state(AppState { pool, ws_tx });

    let settings = config::Settings::load().expect("Failed to load Mimir.toml configuration");
    let bind_addr = format!("0.0.0.0:{}", settings.service.port);
    info!(%bind_addr, "Binding TCP listener");
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    info!("Server started and listening");
    axum::serve(listener, app).await.unwrap();
}
