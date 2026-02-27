use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{FromRow, Sqlite};

#[derive(FromRow, Debug, Serialize, Clone)]
pub struct Account {
    pub ida: u32,
    pub idu: u32,
    pub kind: AccountKind,
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub enum AccountKind {
    Mono,
}

impl sqlx::Type<Sqlite> for AccountKind {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <str as sqlx::Type<Sqlite>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, Sqlite> for AccountKind {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<IsNull, BoxDynError> {
        let s = match self {
            AccountKind::Mono => "Mono",
        };
        <&str as sqlx::Encode<Sqlite>>::encode(s, buf)
    }
}

impl<'r> sqlx::Decode<'r, Sqlite> for AccountKind {
    fn decode(value: <Sqlite as sqlx::Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <&str as sqlx::Decode<Sqlite>>::decode(value)?;
        match s {
            "Mono" => Ok(AccountKind::Mono),
            other => Err(format!("unknown AccountKind: {}", other).into()),
        }
    }
}

/// HTTP request payload for creating an account.
#[derive(Deserialize)]
pub struct CreateAccount {
    pub idu: u32,
    pub kind: AccountKind,
    pub token: String,
}

/// Top-level response from `GET /personal/client-info`.
#[derive(Deserialize, Debug)]
pub struct MonoClientInfo {
    pub accounts: Vec<MonoAccount>,
}

/// A single account entry returned from the Monobank API.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MonoAccount {
    /// The `ida` from our DB — injected after the API call.
    #[serde(skip_deserializing)]
    pub ida: u32,
    pub id: String,
    #[serde(rename = "sendId")]
    pub send_id: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: u32,
    pub balance: u64,
    #[serde(rename = "creditLimit")]
    pub credit_limit: u32,
    #[serde(rename = "maskedPan")]
    pub masked_pan: Vec<String>,
    pub iban: String,
}

/// A row from the `accounts_monitor` table.
#[derive(FromRow, Debug, Serialize, Clone)]
pub struct AccountMonitor {
    pub ida: u32,
    pub external_id: String,
    pub currency_code: u32,
    pub balance: u32,      
    pub credit_limit: u32, 
    pub iban: String,      
    pub masked_pan: String,
    pub kind: AccountKind,
    pub updated_at: Option<DateTime<Utc>>,
    pub last_taken_date: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct StatQueryParams {
    pub from: i64,
    pub to: i64,
}

