use serde::{Deserialize, Serialize};
use serde_with::{serde_as, TimestampSeconds};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
// ------------------ Monobank transaction DTO ------------------
/// Minimal Monobank transaction structure used for mapping to `bills`.
#[derive(Deserialize, Debug)]
pub struct MonobankTransaction {
    pub id: String,
    pub time: i64, // seconds since epoch
    pub description: Option<String>,
    pub mcc: Option<i32>,
    pub hold: Option<bool>,
    pub amount: i64,
    #[serde(rename = "currencyCode")]
    pub currency_code: i32,
    pub balance: i64,
    #[serde(rename = "receiptId")]
    pub receipt_id: Option<String>,
}

#[derive(FromRow, Debug, Deserialize, Serialize, Clone)]
pub struct BankTransaction {
    pub id: String,
    pub external_id: String,
    pub ida: u32,
    pub amount: i64,
    pub currency_code: i32,
    pub description: Option<String>,
    pub mcc: Option<i32>,
    pub hold: Option<bool>,
    pub transaction_time: DateTime<Utc>,
    pub receipt_id: Option<String>,
    pub balance: Option<i64>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct BankTransactionFilter {
    pub ida_list: Option<Vec<String>>,
    pub external_id_list: Option<Vec<String>>,
    pub mcc_list: Option<Vec<i32>>,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub to: DateTime<Utc>,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub from: DateTime<Utc>,
}
