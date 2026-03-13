use crate::transaction::model::{
    BankTransaction, BankTransactionDTO, BankTransactionFilter, BankTransactionTag, TransactionTag,
};
use futures_util::future::join_all;
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Execute, QueryBuilder, Sqlite, SqlitePool};
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracing::{debug, error};

/// Bulk-insert transactions with "INSERT OR IGNORE" to avoid duplicates by PK idt
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
        "INSERT OR IGNORE INTO bank_transaction (idt, ida, external_id, amount, currency_code, description, mcc, hold, transaction_time, receipt_id, balance) ",
    );

    qb.push_values(transactions.iter(), |mut b, transaction| {
        b.push_bind(&transaction.idt)
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

pub async fn get_transactions(
    filter: BankTransactionFilter,
    pool: &SqlitePool,
) -> Result<Vec<BankTransactionDTO>, sqlx::Error> {
    debug!("Selecting bank transactions");

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT t.idt, t.ida, t.external_id, t.amount, t.currency_code,
         t.description, t.mcc, t.hold, t.transaction_time, t.receipt_id, t.balance, a.masked_pan,
         NULL as mcc_description, NULL as currency,
         (SELECT GROUP_CONCAT(tag || '+' || COALESCE(severity, 'primary'), ',') FROM bank_transaction_tag WHERE idt = t.idt) as tags
        FROM bank_transaction t
            LEFT JOIN bank_account_monitor a
            ON t.external_id = a.external_id
            LEFT JOIN bank_account b
            ON a.ida = b.ida
        WHERE ",
    );

    qb.push("t.ida in (");

    if let Some(ida_list) = filter.ida_list {
        if !ida_list.is_empty() {
            let mut separated = qb.separated(", ");
            for ida in ida_list {
                separated.push_bind(ida);
            }
            separated.push_unseparated(")");
        } else {
            qb.push(" select ida from bank_account)");
        }
    } else {
        qb.push(" select ida from bank_account)");
    }

    qb.push(" AND t.transaction_time >= ");
    qb.push_bind(filter.from);
    qb.push(" AND t.transaction_time <= ");
    qb.push_bind(filter.to);

    if let Some(external_id_list) = filter.external_id_list {
        if !external_id_list.is_empty() {
            qb.push(" AND t.external_id IN (");
            let mut separated = qb.separated(", ");
            for eid in external_id_list {
                separated.push_bind(eid.clone());
            }
            separated.push_unseparated(")");
        }
    }

    if let Some(exceptions) = filter.exceptions {
        for ex in exceptions {
            if ex.conditions.is_empty() {
                continue;
            }

            // combinator is e.g. "AND NOT", "AND", "OR NOT", "OR"
            qb.push(format!(" {} (", ex.combinator));

            let mut first = true;
            for cond in ex.conditions {
                // Handle tag field separately via EXISTS subquery (only eq/neq)
                if cond.field == "tag" {
                    match cond.operator.as_str() {
                        "eq" | "neq" => {
                            if !first {
                                qb.push(" AND ");
                            }
                            first = false;

                            // severity is required field in tag
                            if cond.severity.is_none() {
                                continue;
                            }

                            let severity = cond.severity.unwrap();
                            let value = cond.value;

                            if cond.operator == "neq" {
                                qb.push("NOT EXISTS (SELECT 1 FROM bank_transaction_tag WHERE idt = t.idt AND tag = ");
                            } else {
                                qb.push("EXISTS (SELECT 1 FROM bank_transaction_tag WHERE idt = t.idt AND tag = ");
                            }
                            qb.push_bind(value);
                            qb.push(" AND severity = ");
                            qb.push_bind(severity);
                            qb.push(")");
                        }
                        _ => {}
                    }
                    continue;
                }

                let col = match cond.field.as_str() {
                    "amount" => "t.amount",
                    "mcc" => "t.mcc",
                    "currency" => "t.currency_code",
                    "description" => "t.description",
                    "receipt_id" => "t.receipt_id",
                    "external_id" => "t.external_id",
                    _ => continue,
                };

                if !first {
                    qb.push(" AND ");
                }
                first = false;

                match cond.operator.as_str() {
                    "eq" => {
                        qb.push(format!("{} = ", col));
                        qb.push_bind(cond.value.clone());
                    }
                    "neq" => {
                        qb.push(format!("{} != ", col));
                        qb.push_bind(cond.value.clone());
                    }
                    "lt" => {
                        qb.push(format!("{} < ", col));
                        qb.push_bind(cond.value.clone());
                    }
                    "gt" => {
                        qb.push(format!("{} > ", col));
                        qb.push_bind(cond.value.clone());
                    }
                    "lte" => {
                        qb.push(format!("{} <= ", col));
                        qb.push_bind(cond.value.clone());
                    }
                    "gte" => {
                        qb.push(format!("{} >= ", col));
                        qb.push_bind(cond.value.clone());
                    }
                    "startsWith" => {
                        qb.push(format!("{} LIKE ", col));
                        qb.push_bind(format!("{}%", cond.value));
                    }
                    "endsWith" => {
                        qb.push(format!("{} LIKE ", col));
                        qb.push_bind(format!("%{}", cond.value));
                    }
                    "contains" => {
                        qb.push(format!("{} LIKE ", col));
                        qb.push_bind(format!("%{}%", cond.value));
                    }
                    _ => {
                        first = true;
                    }
                }
            }

            qb.push(")");
        }
    }

    qb.push(" order by t.transaction_time desc");

    let query_as = qb.build_query_as::<BankTransactionDTO>();
    let sql = query_as.sql();
    debug!("sql: {sql}");
    query_as.fetch_all(pool).await
}

pub async fn get_mcc(pool: &SqlitePool) -> Result<Vec<u32>, sqlx::Error> {
    debug!("Selecting mcc");

    sqlx::query_scalar("select DISTINCT mcc from bank_transaction")
        .fetch_all(pool)
        .await
}

pub async fn get_transaction_by_id(
    idt: &str,
    pool: &SqlitePool,
) -> Result<BankTransaction, sqlx::Error> {
    sqlx::query_as::<Sqlite, BankTransaction>(
        "SELECT idt, ida, external_id, amount, currency_code, description, mcc, hold, transaction_time, receipt_id, balance
            FROM bank_transaction where idt = $1",
    )
        .bind(idt)
        .fetch_one(pool)
        .await
}

pub async fn get_similar_transaction_tags(
    keys: &[(u32, String)],
    pool: &SqlitePool,
) -> Result<HashMap<(u32, String), HashSet<TransactionTag>>, sqlx::Error> {
    // If no keys provided, nothing to search for
    if keys.is_empty() {
        return Ok(HashMap::new());
    }

    // Build a query that joins tags to transactions and matches any of the (mcc, description) pairs
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT bt.mcc, bt.description, t.tag, t.severity FROM bank_transaction_tag t JOIN bank_transaction bt ON t.idt = bt.idt WHERE ",
    );

    let mut separated = qb.separated(" OR ");
    for (m, d) in keys {
        separated.push("(");
        separated.push("bt.mcc = ");
        separated.push_bind_unseparated(m);
        separated.push_unseparated(" AND bt.description = ");
        separated.push_bind_unseparated(d);
        separated.push_unseparated(")");
    }

    qb.push(" ORDER BY bt.mcc, bt.description, t.tag, t.severity");

    // Rows: (mcc, description, tag, severity)
    let rows: Vec<(i32, String, String, String)> = qb
        .build_query_as::<(i32,String, String, String)>()
        .fetch_all(pool)
        .await?;

    let mut map: HashMap<(u32, String), HashSet<TransactionTag>> = HashMap::new();
    for (mcc_i32, desc, tag, severity) in rows {
        // Convert mcc to u32
        let mcc_u32 = mcc_i32 as u32;
        let key = (mcc_u32, desc);
        map.entry(key)
            .or_default()
            .insert(TransactionTag { tag, severity });
    }

    Ok(map)
}

pub async fn get_similar_transactions(
    idt: &str,
    mcc: bool,
    description: bool,
    pool: &SqlitePool,
) -> Result<Vec<String>, sqlx::Error> {
    // If no filter requested, return empty result
    if !mcc && !description {
        debug!(%idt, "No similarity criteria specified");
        return Ok(vec![]);
    }

    // Fetch the reference transaction; propagate DB error if not found
    let base_tx = get_transaction_by_id(idt, pool).await?;

    // Build dynamic WHERE clause: we combine requested fields with OR (mcc OR description)
    let mut qb = QueryBuilder::new("SELECT idt FROM bank_transaction WHERE ");

    let mut first = true;

    if mcc {
        if !first {
            qb.push(" AND ");
        }
        // handle NULL mcc explicitly
        match base_tx.mcc {
            Some(m) => {
                qb.push("(mcc = ");
                qb.push_bind(m);
                qb.push(")");
            }
            None => {
                qb.push("(mcc IS NULL)");
            }
        }
        first = false;
    }

    if description {
        if !first {
            qb.push(" AND ");
        }
        match base_tx.description {
            Some(ref d) => {
                qb.push("(description = ");
                qb.push_bind(d);
                qb.push(")");
            }
            None => {
                qb.push("(description IS NULL)");
            }
        }
    }

    qb.push(" ORDER BY transaction_time DESC");

    // Fetch single-column rows as (String,) then map to Vec<String>
    let rows: Vec<(String,)> = qb.build_query_as::<(String,)>().fetch_all(pool).await?;
    let ids: Vec<String> = rows.into_iter().map(|(s,)| s).collect();
    Ok(ids)
}

pub async fn add_transaction_tags(
    tags: &[BankTransactionTag],
    pool: &SqlitePool,
) -> Result<Vec<BankTransactionTag>, sqlx::Error> {
    if tags.is_empty() {
        return Ok(vec![]);
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("INSERT OR IGNORE INTO bank_transaction_tag (idt, tag, severity) ");

    #[derive(Debug)]
    struct TagEntry {
        idt: String,
        tag: String,
        severity: String,
    }

    let entries: Vec<TagEntry> = tags
        .iter()
        .flat_map(|btag| {
            btag.tags.iter().map(move |t| TagEntry {
                idt: btag.idt.clone(),
                tag: t.tag.clone(),
                severity: t.severity.clone(),
            })
        })
        .collect();

    qb.push_values(entries.iter(), |mut b, t| {
        b.push_bind(&t.idt).push_bind(&t.tag).push_bind(&t.severity);
    });

    qb.build().execute(pool).await?;

    let mut handles: Vec<JoinHandle<BankTransactionTag>> = Vec::with_capacity(tags.len());

    // Spawn tasks and collect their JoinHandles
    for btag in tags.iter() {
        let idt = btag.idt.clone();
        let pool = pool.clone();
        let handle = tokio::spawn(async move { get_transaction_tags_by_idt(idt, &pool).await });
        handles.push(handle);
    }

    // Await all tasks to complete and collect results
    let results = join_all(handles).await;

    let mut tags: Vec<BankTransactionTag> = Vec::with_capacity(tags.len());
    for res in results {
        match res {
            Ok(v) => tags.push(v),
            Err(e) => error!(%e, "Transaction tag fetch task panicked"),
        }
    }

    Ok(tags)
}

pub async fn delete_transaction_tags(
    tags: &[BankTransactionTag],
    pool: &SqlitePool,
) -> Result<Vec<BankTransactionTag>, sqlx::Error> {
    let entries: Vec<(String, String, String)> = tags
        .iter()
        .flat_map(|btag| {
            btag.tags
                .iter()
                .map(move |t| (btag.idt.clone(), t.tag.clone(), t.severity.clone()))
        })
        .collect();

    if !entries.is_empty() {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("DELETE FROM bank_transaction_tag WHERE ");

        let mut separated = qb.separated(" OR ");
        for (idt, tag, severity) in &entries {
            separated.push("(idt = ");
            separated.push_bind_unseparated(idt);
            separated.push_unseparated(" AND tag = ");
            separated.push_bind_unseparated(tag);
            separated.push_unseparated(" AND severity = ");
            separated.push_bind_unseparated(severity);
            separated.push_unseparated(")");
        }

        qb.build().execute(pool).await?;
    }

    let mut handles: Vec<JoinHandle<BankTransactionTag>> = Vec::with_capacity(tags.len());

    // Spawn tasks and collect their JoinHandles
    for btag in tags.iter() {
        let idt = btag.idt.clone();
        let pool = pool.clone();
        let handle = tokio::spawn(async move { get_transaction_tags_by_idt(idt, &pool).await });
        handles.push(handle);
    }

    // Await all tasks to complete and collect results
    let results = join_all(handles).await;

    let mut tags: Vec<BankTransactionTag> = Vec::with_capacity(tags.len());
    for res in results {
        match res {
            Ok(v) => tags.push(v),
            Err(e) => error!(%e, "Transaction tag fetch task panicked"),
        }
    }

    Ok(tags)
}

pub async fn get_transaction_tags_by_idt(idt: String, pool: &SqlitePool) -> BankTransactionTag {
    let sql = "SELECT DISTINCT tag, severity FROM bank_transaction_tag where idt = $1";
    let tags = sqlx::query_as::<Sqlite, TransactionTag>(sql)
        .bind(&idt)
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| {
            error!(%e, "Failed to fetch transaction tags for idt");
            vec![]
        });

    BankTransactionTag { idt, tags }
}

pub async fn get_transaction_tags(pool: &SqlitePool) -> Result<Vec<TransactionTag>, sqlx::Error> {
    let sql = "SELECT DISTINCT tag, severity FROM bank_transaction_tag";
    sqlx::query_as::<Sqlite, TransactionTag>(sql)
        .fetch_all(pool)
        .await
}

pub async fn get_transaction_tags_names(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    let sql = "SELECT DISTINCT tag FROM bank_transaction_tag";
    sqlx::query_scalar(sql).fetch_all(pool).await
}
