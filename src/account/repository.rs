use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tracing::error;

use super::model::{Account, AccountKind, AccountMonitor, MonoAccount};

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
pub async fn insert_accounts_monitor(
    accounts: &[MonoAccount],
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    if accounts.is_empty() {
        return Ok(());
    }

    let mut query_builder: QueryBuilder<Sqlite> =
        QueryBuilder::new("INSERT INTO accounts_monitor (ida, external_id) ");

    query_builder.push_values(accounts.iter(), |mut b, account| {
        b.push_bind(account.ida).push_bind(account.id.clone());
    });

    query_builder.build().execute(pool).await.map(|_| ())
}

/// Fetch all rows from `accounts_monitor`.
pub async fn find_all_monitors_by_idu(
    idu: u32,
    pool: &SqlitePool,
) -> Result<Vec<AccountMonitor>, sqlx::Error> {
    sqlx::query_as::<Sqlite, AccountMonitor>(
        "SELECT ida, external_id, updated_at FROM accounts_monitor where ida in (\
        select ida from accounts where idu = $1)",
    )
    .bind(idu)
    .fetch_all(pool)
    .await
}
