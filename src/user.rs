use crate::error::AppError;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, SqlitePool};
use tracing::debug;

#[derive(FromRow, Debug, Serialize)]
pub struct User {
    pub idu: u32,
    pub name: String,
}

// the input to our `create_user` handler
#[derive(Deserialize)]
pub struct CreateUser {
    pub name: String,
}

pub async fn create_user(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateUser>,
) -> Result<(StatusCode, Json<User>), AppError> {
    let user =
        sqlx::query_as::<Sqlite, User>("INSERT INTO suser (name) VALUES ($1) RETURNING idu, name")
            .bind(payload.name)
            .fetch_one(&pool)
            .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn delete_user(
    State(pool): State<SqlitePool>,
    Path(idu): Path<u32>,
) -> Result<StatusCode, AppError> {
    debug!(%idu, "Deleting user");
    sqlx::query("DELETE FROM suser WHERE idu = ?")
        .bind(idu)
        .execute(&pool)
        .await?;
    Ok(StatusCode::OK)
}

pub async fn get_users(State(pool): State<SqlitePool>) -> Result<Json<Vec<User>>, AppError> {
    let users = sqlx::query_as::<Sqlite, User>("SELECT idu, name from suser")
        .fetch_all(&pool)
        .await?;
    Ok(Json(users))
}
