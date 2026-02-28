use crate::error::AppError;
use crate::mcc_data::{MccEntry, lookup_mcc};
use crate::transaction::{BankTransaction, repository};
use axum::Json;
use axum::extract::{Path, State};
use sqlx::SqlitePool;
use tracing::{debug, error, instrument};

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

#[instrument(skip(pool))]
pub async fn get_mcc_by_idu(
    Path(idu): Path<u32>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<MccEntry>>, AppError> {
    debug!(idu, "Fetching mcc by user id");

    let mcc_list = repository::get_mcc_by_idu(idu, &pool).await?;
    let mut result: Vec<MccEntry> = vec![];

    for mcc in mcc_list.iter() {
        match lookup_mcc(&mcc.to_string()) {
            None => {
                error!(mcc, "Merchant Category Code not found")
            }
            Some(m) => result.push(m.clone()),
        }
    }

    Ok(Json(result))
}
