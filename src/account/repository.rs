use super::model::{
    Account, AccountKind, AccountMonitor, AccountMonitorStatus, MonoAccount, NewAccount,
    UpdateAccount,
};
use secrecy::ExposeSecret;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool};
use tracing::{debug, error, info};

/// Fetch every account row from the DB.
pub async fn find_all(pool: &SqlitePool) -> Vec<Account> {
    debug!("Selecting all accounts from DB");
    sqlx::query_as::<Sqlite, Account>("SELECT ida, kind, name FROM bank_account")
        .fetch_all(pool)
        .await
        .unwrap_or_else(|err| {
            error!("SQL error fetching all accounts: {}", err);
            vec![]
        })
}

/// Insert a new account row and return the created record.
pub async fn insert(account: NewAccount, pool: &SqlitePool) -> Result<Account, sqlx::Error> {
    debug!("Inserting new account");
    sqlx::query_as::<Sqlite, Account>(
        "INSERT INTO bank_account (kind, token, name) VALUES ($1, $2, $3) RETURNING ida, kind, name",
    )
    .bind(account.kind)
    .bind(account.token.expose_secret())
    .bind(account.name)
    .fetch_one(pool)
    .await
}

/// Bulk-insert a slice of `MonoAccount`s into `accounts_monitor`.
pub async fn insert_mono_accounts_monitor(
    accounts: &[MonoAccount],
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    if accounts.is_empty() {
        debug!("No mono accounts to insert");
        return Ok(());
    }

    debug!(
        count = accounts.len(),
        "Inserting mono accounts into accounts_monitor"
    );

    for account in accounts {
        debug!(external_id = %account.id, ida = account.ida, "Inserting monitor row");
        sqlx::query(
            "INSERT INTO bank_account_monitor (ida, external_id, currency_code, balance, credit_limit, iban, masked_pan, kind, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(external_id) DO NOTHING",
        )
        .bind(account.ida)
        .bind(&account.id)
        .bind(account.currency_code as i64)
        .bind(account.balance as i64)
        .bind(account.credit_limit as i64)
        .bind(&account.iban)
        .bind(account.masked_pan.join(","))
        .bind("Mono")
        .bind(AccountMonitorStatus::Never.to_string())
        .execute(pool)
        .await?;
    }

    info!("Inserted mono accounts monitors");
    Ok(())
}

/// Fetch all rows from `accounts_monitor`.
pub async fn fetch_all_monitors(pool: &SqlitePool) -> Vec<AccountMonitor> {
    debug!("Selecting account monitors");
    sqlx::query_as::<Sqlite, AccountMonitor>(
        "SELECT ida, external_id, currency_code, balance, credit_limit, iban, masked_pan, kind, updated_at, last_taken_date, status
        FROM bank_account_monitor",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|err| {
        error!("SQL error fetching all accounts: {}", err);
        vec![]
    })
}

/// Fetch token for account by ida
pub async fn get_token_by_ida(ida: u32, pool: &SqlitePool) -> Result<String, sqlx::Error> {
    debug!(ida, "Selecting token by ida");
    let token: String = sqlx::query_scalar("SELECT token FROM bank_account WHERE ida = ?")
        .bind(ida)
        .fetch_one(pool)
        .await?;
    Ok(token)
}

/// Update accounts_monitor status
pub async fn update_monitor_status(
    ida: u32,
    external_id: String,
    status: AccountMonitorStatus,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    debug!(ida, "Updating monitor status");
    sqlx::query("UPDATE bank_account_monitor SET status = ? WHERE ida = ? and external_id = ?")
        .bind(status.to_string())
        .bind(ida)
        .bind(external_id)
        .execute(pool)
        .await
        .map(|_| ())
}
/// Update accounts_monitor timestamps after successful fetch
pub async fn update_monitor_timestamps(
    ida: u32,
    external_id: &str,
    updated_at: DateTime<Utc>,
    maybe_last_taken_date: Option<DateTime<Utc>>,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    debug!(ida, "Updating monitor timestamps");
    if let Some(lt) = maybe_last_taken_date {
        sqlx::query("UPDATE bank_account_monitor SET updated_at = ?, last_taken_date = ? WHERE ida = ? and external_id = ?")
            .bind(updated_at)
            .bind(lt)
            .bind(ida)
            .bind(external_id)
            .execute(pool)
            .await
            .map(|_| ())
    } else {
        sqlx::query(
            "UPDATE bank_account_monitor SET updated_at = ? WHERE ida = ? and external_id = ?",
        )
        .bind(updated_at)
        .bind(ida)
        .bind(external_id)
        .execute(pool)
        .await
        .map(|_| ())
    }
}

pub async fn fetch_monitor_by_external_id(
    external_id: &str,
    pool: &SqlitePool,
) -> Result<Option<AccountMonitor>, sqlx::Error> {
    debug!(%external_id, "Selecting account monitor by external_id");
    sqlx::query_as::<Sqlite, AccountMonitor>(
        "SELECT ida, external_id, currency_code, balance, credit_limit, iban, masked_pan, kind, updated_at, last_taken_date, status
        FROM bank_account_monitor where external_id = $1",
    )
        .bind(external_id)
        .fetch_optional(pool)
        .await
}

pub async fn delete_account(ida: u32, pool: &SqlitePool) -> Result<(), sqlx::Error> {
    debug!(%ida, "Deleting account");
    sqlx::query("DELETE FROM bank_account WHERE ida = ?")
        .bind(ida)
        .execute(pool)
        .await
        .map(|_| ())
}

pub async fn update_account(
    ida: u32,
    payload: UpdateAccount,
    pool: &SqlitePool,
) -> Result<Account, sqlx::Error> {
    debug!(ida, "Updating account");

    let mut set_clauses: Vec<&str> = Vec::new();

    if payload.name.is_some() {
        set_clauses.push("name = ?");
    }
    if payload.token.is_some() {
        set_clauses.push("token = ?");
    }

    if set_clauses.is_empty() {
        debug!(ida, "Nothing to update, fetching existing account");
        return sqlx::query_as::<Sqlite, Account>(
            "SELECT ida, kind, name FROM bank_account WHERE ida = ?",
        )
        .bind(ida)
        .fetch_one(pool)
        .await;
    }

    let sql = format!(
        "UPDATE bank_account SET {} WHERE ida = ? RETURNING ida, kind, name",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query_as::<Sqlite, Account>(&sql);

    if let Some(name) = payload.name {
        query = query.bind(name);
    }
    if let Some(token) = payload.token {
        query = query.bind(token.expose_secret().to_owned());
    }

    query.bind(ida).fetch_one(pool).await
}
