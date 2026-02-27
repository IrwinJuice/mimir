use crate::error::AppError;
use crate::transaction::{repository, BankTransaction};
use axum::extract::{Path, State};
use axum::Json;
use sqlx::SqlitePool;
use tracing::{debug, instrument};

#[instrument(skip(pool))]
pub async fn get_transactions_by_ida(
    Path(ida): Path<u32>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<BankTransaction>>, AppError> {
    debug!(ida, "Fetching bank transactions by account id");

    let transactions = repository::get_transactions_by_ida(ida, &pool).await?;
    debug!(count = transactions.len(), "Found transactions");
    Ok(Json(transactions))
}
