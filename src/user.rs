use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, SqlitePool};
use crate::error::AppError;

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
    let user = sqlx::query_as::<Sqlite, User>(
        "INSERT INTO users (name) VALUES ($1) RETURNING idu, name",
    )
    .bind(payload.name)
    .fetch_one(&pool)
    .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn get_users(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<User>>, AppError> {
    let users = sqlx::query_as::<Sqlite, User>("SELECT idu, name from users")
        .fetch_all(&pool)
        .await?;
    Ok(Json(users))
}





