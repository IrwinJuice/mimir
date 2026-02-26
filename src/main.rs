mod account;
mod error;
mod tracing_config;
mod user;
mod utils;

use crate::account::handler::{add_account, get_accounts_by_idu, get_accounts_monitors, update_accounts_stat};
use crate::user::{create_user, get_users};
use axum::http::{header, HeaderValue, Method};
use axum::routing::{get, get_service, put};
use axum::Router;
use sqlx::SqlitePool;
use std::fs::File;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{debug, info};
use tracing_log::LogTracer;

#[tokio::main]
async fn main() {
    // initialize tracing with environment-controlled level
    LogTracer::init().expect("Failed to set logger");
    tracing_config::init();

    info!("Starting bills-service");

    let _ = File::create_new("bills.db");

    debug!("Connecting to SQLite database at bills.db");
    let pool = SqlitePool::connect("bills.db").await.unwrap();

    debug!("Running database migrations");
    sqlx::migrate!().run(&pool).await.unwrap();
    info!("Database connected and migrations applied");

    // Serve Angular app from the dist folder, falling back to index.html for SPA routing
    let serve_dir = ServeDir::new("./dist/bills-client/browser")
        .not_found_service(ServeFile::new("./dist/bills-client/browser/index.html"));

    // build our application with a route
    let app = Router::new()
        // Add API routes here, e.g.:
        .route(
            "/bills/api/users",
            get(get_users).post(create_user), // delete(delete_users),
        )
        .route(
            "/bills/api/users/{idu}/accounts",
            get(get_accounts_by_idu).post(add_account), // delete(delete_users),
        )
        .route(
            "/bills/api/users/{idu}/accounts/stat",
            put(update_accounts_stat),
        )
        .route(
            "/bills/api/users/{idu}/monitors",
            get(get_accounts_monitors),
        )
        // Serve the Angular SPA under /bills
        .nest_service("/bills", serve_dir.clone())
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
        .with_state(pool.clone());

    let bind_addr = "0.0.0.0:42000";
    info!(%bind_addr, "Binding TCP listener");
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .unwrap();
    info!("Server started and listening");
    axum::serve(listener, app).await.unwrap();
}
