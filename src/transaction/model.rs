use serde::{Deserialize, Serialize};
use serde_with::{TimestampSeconds, serde_as};
use sqlx::FromRow;
use sqlx::Row;
use sqlx::types::chrono::{DateTime, Utc};

/// A single tag.
/// Severity values: primary | secondary | success | info | warn | danger | contrast
#[derive(FromRow, Debug, Deserialize, Serialize, Clone)]
pub struct TransactionTag {
    pub tag: String,
    pub severity: String,
}
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BankTransactionDTO {
    pub idt: String,
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

    pub masked_pan: Option<String>,
    pub mcc_description: Option<String>,
    pub currency: Option<String>,

    pub tags: Vec<TransactionTag>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for BankTransactionDTO {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        // tags column contains "tag1:severity1,tag2:severity2,..." where severity may be empty
        let tags_raw: Option<String> = row.try_get("tags")?;
        let tags = match tags_raw {
            Some(s) if !s.is_empty() => s
                .split(',')
                .map(|entry| {
                    let mut parts = entry.splitn(2, '+');
                    let tag = parts.next().unwrap_or("fail_to_get_tag").to_string();
                    let severity = parts.next().unwrap_or("primary").to_string();
                    TransactionTag { tag, severity }
                })
                .collect(),
            _ => vec![],
        };
        Ok(Self {
            idt: row.try_get("idt")?,
            external_id: row.try_get("external_id")?,
            ida: row.try_get("ida")?,
            amount: row.try_get("amount")?,
            currency_code: row.try_get("currency_code")?,
            description: row.try_get("description")?,
            mcc: row.try_get("mcc")?,
            hold: row.try_get("hold")?,
            transaction_time: row.try_get("transaction_time")?,
            receipt_id: row.try_get("receipt_id")?,
            balance: row.try_get("balance")?,
            masked_pan: row.try_get("masked_pan")?,
            mcc_description: row.try_get("mcc_description")?,
            currency: row.try_get("currency")?,
            tags,
        })
    }
}

#[derive(FromRow, Debug, Deserialize, Serialize, Clone)]
pub struct BankTransaction {
    pub idt: String,
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

#[derive(Deserialize, Debug)]
pub struct FilterCondition {
    pub field: String, // "amount" | "currency" | "description" | "receipt_id" | "mcc"
    pub operator: String, // "eq" | "neq" | "lt" | "gt" | "lte" | "gte" | "startsWith" | "endsWith" | "contains"
    pub value: String,
    pub severity: Option<String> // primary | secondary | success | info | warn | danger | contrast
}

// combinator: "AND NOT" | "AND" | "OR NOT" | "OR"
#[derive(Deserialize, Debug)]
pub struct FilterException {
    pub combinator: String,
    pub conditions: Vec<FilterCondition>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct BankTransactionFilter {
    pub ida_list: Option<Vec<u32>>,
    pub external_id_list: Option<Vec<String>>,
    pub exceptions: Option<Vec<FilterException>>,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub from: DateTime<Utc>,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub to: DateTime<Utc>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BankTransactionTag {
    pub idt: String,
    pub tags: Vec<TransactionTag>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MagicBankTransactionTag {
    pub idt: String,
    pub tags: Vec<TransactionTag>,
    pub by_mcc: bool,
    pub by_description: bool,
}
