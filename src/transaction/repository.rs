use crate::transaction::model::{BankTransaction, BankTransactionDTO, BankTransactionFilter};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tracing::debug;
use crate::utils::datetime::DateTimeUtc;

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

pub async fn get_transactions(
    idu: u32,
    filter: BankTransactionFilter,
    pool: &SqlitePool,
) -> Result<Vec<BankTransactionDTO>, sqlx::Error> {
    debug!("Selecting bank transactions");

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT t.id, t.ida, t.external_id, t.amount, t.currency_code,
         t.description, t.mcc, t.hold, t.transaction_time, t.receipt_id, t.balance, a.masked_pan,
         NULL as mcc_description, NULL as currency
        FROM bank_transaction t
            LEFT JOIN bank_account_monitor a
            ON t.external_id = a.external_id
        WHERE t.ida in ("
    );

    if let Some(ida_list) = filter.ida_list {
        if !ida_list.is_empty() {
            let mut separated = qb.separated(", ");
            for ida in ida_list {
                separated.push_bind(ida);
            }
            separated.push_unseparated(")");
        } else {
            qb.push(" select ida from bank_account where idu = ");
            qb.push_bind(idu);
            qb.push(")");
        }
    } else {
        qb.push(" select ida from bank_account where idu = ");
        qb.push_bind(idu);
        qb.push(")");
    }
    qb.push(" AND t.transaction_time >= ");
    qb.push_bind(filter.from);
    qb.push(" AND t.transaction_time <= ");
    qb.push_bind(filter.to);

    if let Some(mcc_list) = filter.mcc_list {
        if !mcc_list.is_empty() {
            qb.push(" AND t.mcc IN (");
            let mut separated = qb.separated(", ");
            for mcc in mcc_list {
                separated.push_bind(mcc);
            }
            separated.push_unseparated(")");
        }
    }

    if let Some(ex_id_list) = filter.external_id_list {
        if !ex_id_list.is_empty() {
            qb.push(" AND t.external_id IN (");
            let mut separated = qb.separated(", ");
            for mcc in ex_id_list {
                separated.push_bind(mcc);
            }
            separated.push_unseparated(")");
        }
    }
    qb.push(" order by t.transaction_time desc");

    qb.build_query_as::<BankTransactionDTO>().fetch_all(pool).await
}

pub async fn get_mcc_by_idu(idu: u32, pool: &SqlitePool) -> Result<Vec<u32>, sqlx::Error> {
    debug!(%idu, "Selecting mcc by user id");

    sqlx::query_scalar("select DISTINCT mcc from bank_transaction where ida in ( select ida from bank_account where idu = $1)")
        .bind(idu)
        .fetch_all(pool)
        .await
}
