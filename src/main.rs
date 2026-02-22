mod account;
mod error;
mod user;
mod utils;

use crate::account::handler::{add_account, get_accounts_by_idu, update_account_stat};
use crate::user::{create_user, get_users};
use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{get, get_service, patch, post};
use sqlx::SqlitePool;
use std::fs::File;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let _ = File::create_new("bills.db");

    let pool = SqlitePool::connect("bills.db").await.unwrap();

    sqlx::migrate!().run(&pool).await.unwrap();

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
            patch(update_account_stat),
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
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:42000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
