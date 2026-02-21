use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, SqlitePool};
use crate::internal_error;


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

// impl User {
//     pub fn new(idu: u32, name: String) -> Self {
//         Self { idu, name }
//     }
// }

// pub async fn create_user(
//     Json(payload): Json<CreateUser>,
// ) -> (StatusCode, Json<User>) {
//
//     (StatusCode::CREATED, Json(user))
// }

pub async fn get_users(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
    sqlx::query_as::<Sqlite, User>("SELECT idu, name from users")
        .fetch_all(&pool)
        .await
        .map(Json)
        .map_err(internal_error)
}





