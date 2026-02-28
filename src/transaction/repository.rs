use crate::transaction::model::BankTransaction;
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tracing::debug;

/// Bulk-insert transactions with "INSERT OR IGNORE" to avoid duplicates by PK id
pub async fn insert_transactions(
    transactions: &[BankTransaction],
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    if transactions.is_empty() {
        debug!("No bills to insert");
        return Ok(());
    }

    debug!(count = transactions.len(), "Inserting bills");

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "INSERT OR IGNORE INTO bank_transaction (id, ida, external_id, amount, currency_code, description, mcc, hold, transaction_time, receipt_id, balance) ",
    );

    qb.push_values(transactions.iter(), |mut b, transaction| {
        b.push_bind(&transaction.id)
            .push_bind(transaction.ida)
            .push_bind(transaction.external_id.clone())
            .push_bind(transaction.amount)
            .push_bind(transaction.currency_code)
            .push_bind(transaction.description.clone())
            .push_bind(transaction.mcc)
            .push_bind(transaction.hold)
            .push_bind(transaction.transaction_time)
            .push_bind(transaction.receipt_id.clone())
            .push_bind(transaction.balance);
    });

    debug!("Executing bills insert statement");
    qb.build()
        .execute(pool)
        .await
        .map(|_: SqliteQueryResult| ())
}

pub async fn get_transactions_by_ida(
    ida: u32,
    pool: &SqlitePool,
) -> Result<Vec<BankTransaction>, sqlx::Error> {
    debug!(%ida, "Selecting bank transactions by account id");
    sqlx::query_as::<Sqlite, BankTransaction>(
        "SELECT id, ida, external_id, amount, currency_code, description, mcc, hold, transaction_time, receipt_id, balance
        FROM bank_transaction where ida = $1",
    )
        .bind(ida)
        .fetch_all(pool)
        .await
}

pub async fn get_mcc_by_idu(
    idu: u32,
    pool: &SqlitePool,
) -> Result<Vec<u32>, sqlx::Error> {
    debug!(%idu, "Selecting mcc by user id");

    sqlx::query_scalar("select DISTINCT mcc from bank_transaction where ida in ( select ida from bank_account where idu = $1)")
        .bind(idu)
        .fetch_all(pool)
        .await
}
