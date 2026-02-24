use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::types::chrono::{DateTime, Utc};
use tracing::error;

use super::model::{Account, AccountKind, AccountMonitor, MonoAccount};
use crate::account::model::NewBill;

/// Fetch every account row from the DB.
pub async fn find_all(pool: &SqlitePool) -> Vec<Account> {
    sqlx::query_as::<Sqlite, Account>("SELECT ida, idu, kind, token FROM accounts")
        .fetch_all(pool)
        .await
        .unwrap_or_else(|err| {
            error!("SQL error fetching all accounts: {}", err);
            vec![]
        })
}

/// Fetch all accounts belonging to a specific user.
pub async fn find_by_idu(idu: u32, pool: &SqlitePool) -> Result<Vec<Account>, sqlx::Error> {
    sqlx::query_as::<Sqlite, Account>("SELECT ida, idu, kind, token FROM accounts WHERE idu = $1")
        .bind(idu)
        .fetch_all(pool)
        .await
}

/// Insert a new account row and return the created record.
pub async fn insert(
    idu: u32,
    kind: AccountKind,
    token: String,
    pool: &SqlitePool,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<Sqlite, Account>(
        "INSERT INTO accounts (idu, kind, token) VALUES ($1, $2, $3) RETURNING ida, idu, kind, token",
    )
    .bind(idu)
    .bind(kind)
    .bind(token)
    .fetch_one(pool)
    .await
}

/// Bulk-insert a slice of `MonoAccount`s into `accounts_monitor`.
pub async fn insert_mono_accounts_monitor(
    accounts: &[MonoAccount],
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    if accounts.is_empty() {
        return Ok(());
    }

    let mut query_builder: QueryBuilder<Sqlite> =
        QueryBuilder::new("INSERT INTO accounts_monitor (ida, external_id, currency_code, balance, credit_limit, iban, masked_pan) VALUES ");

    query_builder.push_values(accounts.iter(), |mut b, account| {
        b.push_bind(account.ida)
            .push_bind(account.id.clone())
            .push_bind(account.currency_code as i64)
            .push_bind(account.balance as i64)
            .push_bind(account.credit_limit as i64)
            .push_bind(account.iban.clone())
            .push_bind(account.masked_pan.join(","));
    });

    query_builder.build().execute(pool).await.map(|_| ())
}

/// Fetch all rows from `accounts_monitor`.
pub async fn find_all_monitors_by_idu(
    idu: u32,
    pool: &SqlitePool,
) -> Result<Vec<AccountMonitor>, sqlx::Error> {
    sqlx::query_as::<Sqlite, AccountMonitor>(
        "SELECT ida, external_id, currency_code, balance, credit_limit, iban, masked_pan, kind, updated_at, last_taken_date FROM accounts_monitor where ida in (\
        select ida from accounts where idu = $1)",
    )
    .bind(idu)
    .fetch_all(pool)
    .await
}

/// Fetch token for account by ida
pub async fn get_token_by_ida(ida: u32, pool: &SqlitePool) -> Result<String, sqlx::Error> {
    let token: String = sqlx::query_scalar("SELECT token FROM accounts WHERE ida = ?")
        .bind(ida)
        .fetch_one(pool)
        .await?;
    Ok(token)
}

/// Bulk-insert bills with "INSERT OR IGNORE" to avoid duplicates by PK id
pub async fn insert_bills(bills: &[NewBill], pool: &SqlitePool) -> Result<(), sqlx::Error> {
    if bills.is_empty() {
        return Ok(());
    }

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("INSERT OR IGNORE INTO bills (id, ida, amount, currency_code, description, mcc, hold, transaction_time, receipt_id, balance) ");

    qb.push_values(bills.iter(), |mut b, bill| {
        b.push_bind(&bill.id)
            .push_bind(bill.ida)
            .push_bind(bill.amount)
            .push_bind(bill.currency_code)
            .push_bind(bill.description.clone())
            .push_bind(bill.mcc)
            .push_bind(bill.hold)
            .push_bind(bill.transaction_time)
            .push_bind(bill.receipt_id.clone())
            .push_bind(bill.balance);
    });

    qb.build().execute(pool).await.map(|_: SqliteQueryResult| () )
}

/// Update accounts_monitor timestamps after successful fetch
pub async fn update_monitor_timestamps(
    ida: u32,
    updated_at: DateTime<Utc>,
    maybe_last_taken_date: Option<DateTime<Utc>>,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    if let Some(lt) = maybe_last_taken_date {
        sqlx::query("UPDATE accounts_monitor SET updated_at = ?, last_taken_date = ? WHERE ida = ?")
            .bind(updated_at)
            .bind(lt)
            .bind(ida)
            .execute(pool)
            .await
            .map(|_| ())
    } else {
        sqlx::query("UPDATE accounts_monitor SET updated_at = ? WHERE ida = ?")
            .bind(updated_at)
            .bind(ida)
            .execute(pool)
            .await
            .map(|_| ())
    }
}
