use super::model::{
    Account, AccountKind, AccountMonitor, AccountMonitorStatus, MonoAccount, MonoClientInfo,
    NewAccount, StatQueryParams, UpdateAccount,
};
use super::repository;
use crate::account::repository::{get_highest_transaction_time, update_monitor_status};
use crate::error::AppError;
use crate::transaction;
use crate::transaction::model::{BankTransactionTag, TransactionTag};
use crate::transaction::repository::add_transaction_tags;
use crate::transaction::{BankTransaction, MonobankTransaction};
use crate::utils::datetime::DateTimeUtc;
use crate::ws_handler::WsTx;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::USER_AGENT;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, instrument, warn};

/// POST /mimir/api/accounts
#[instrument(skip(pool))]
pub async fn add_account(
    State(pool): State<SqlitePool>,
    Json(payload): Json<NewAccount>,
) -> Result<(StatusCode, Json<Account>), AppError> {
    debug!(account_name = payload.name, "Adding account");
    let account = repository::insert(payload, &pool).await?;
    info!(ida = account.ida, "Account created");
    Ok((StatusCode::CREATED, Json(account)))
}

/// GET /mimir/api/accounts
#[instrument(skip(pool))]
pub async fn get_accounts(
    State(pool): State<SqlitePool>,
) -> Result<(StatusCode, Json<Vec<Account>>), AppError> {
    let accounts = repository::find_all(&pool).await;
    info!(accounts = accounts.len(), "Found");
    Ok((StatusCode::CREATED, Json(accounts)))
}

/// DELETE /mimir/api/account/:ida
#[instrument(skip(pool))]
pub async fn delete_account(
    State(pool): State<SqlitePool>,
    Path(ida): Path<u32>,
) -> Result<StatusCode, AppError> {
    debug!(ida = ida, "Delete account");
    repository::delete_account(ida, &pool).await?;
    info!(ida = ida, "Account deleted");
    Ok(StatusCode::OK)
}

/// PUT /mimir/api/account/:ida
#[instrument(skip(pool))]
pub async fn update_account(
    State(pool): State<SqlitePool>,
    Path(ida): Path<u32>,
    Json(payload): Json<UpdateAccount>,
) -> Result<(StatusCode, Json<Account>), AppError> {
    debug!(ida = ida, "Delete account");

    let account = repository::update_account(ida, payload, &pool).await?;
    info!(ida = account.ida, "Account created");
    Ok((StatusCode::OK, Json(account)))
}

/// GET /mimir/api/monitors
#[instrument(skip(pool))]
pub async fn get_account_monitor(
    Path(external_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<AccountMonitor>, AppError> {
    debug!(external_id, "Fetching monitor by external_id");
    match repository::fetch_monitor_by_external_id(&external_id, &pool).await? {
        Some(monitor) => {
            debug!("Found monitor");
            Ok(Json(monitor))
        }
        None => {
            let message = format!("Not Found monitor with external_id {external_id}");
            Err(AppError::NotFound(message))
        }
    }
}

/// GET /mimir/api/monitors
#[instrument(skip(pool))]
pub async fn get_accounts_monitors(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<AccountMonitor>>, AppError> {
    debug!("Fetching monitors");
    let monitors = get_monitors(&pool).await;
    debug!(count = monitors.len(), "Found monitors");
    Ok(Json(monitors))
}

/// PUT /mimir/api/accounts/stat
#[instrument(skip(pool, params, ws_tx))]
pub async fn update_accounts_stat(
    Query(params): Query<StatQueryParams>,
    State(pool): State<SqlitePool>,
    State(ws_tx): State<WsTx>,
) -> Result<(StatusCode, Json<Vec<AccountMonitor>>), AppError> {
    info!(from = %params.from, to = %params.to, "Updating account stat");
    sniff_accounts(&pool).await;
    let monitors = get_monitors(&pool).await;

    debug!("monitors_get_by_idu {:?}", &monitors);

    let monitors_for_response = monitors.clone();

    // Query params mapping: `from` = last_taken_date, `to` = updated_at
    let DateTimeUtc(req_last_taken) = params.from.clone().try_into().map_err(|e| {
        error!(from = %params.from, "Invalid 'from' datetime: {}", e);
        AppError::BadRequest(e)
    })?;

    let DateTimeUtc(req_updated_at) = params.to.clone().try_into().map_err(|e| {
        error!(to = %params.to, "Invalid 'to' datetime: {}", e);
        AppError::BadRequest(e)
    })?;

    // Spawn background task
    tokio::spawn(async move {
        let monobank: Vec<AccountMonitor> = monitors
            .iter()
            .filter(|m| m.kind == AccountKind::Mono)
            .cloned()
            .collect();

        update_mono_accounts_stat(monobank, req_last_taken, req_updated_at, &pool, &ws_tx).await;
    });

    Ok((StatusCode::OK, Json(monitors_for_response)))
}

async fn update_mono_accounts_stat(
    monitors: Vec<AccountMonitor>,
    req_last_taken: DateTime<Utc>,
    req_updated_at: DateTime<Utc>,
    pool: &SqlitePool,
    ws_tx: &WsTx,
) {
    info!(
        "Sleeping 2s to end http call with monitors, to prevent racing condition when monitor is still undefined, but ws event has already arrived."
    );
    sleep(Duration::from_secs(2)).await;
    for monitor in monitors {
        if let Err(e) = update_monitor_status(
            monitor.ida,
            monitor.external_id.clone(),
            AccountMonitorStatus::Pending,
            pool,
        )
        .await
        {
            error!(
                ida = monitor.ida,
                external_id = monitor.external_id.clone(),
                ?e,
                "Monitor status update failed"
            );
        }

        let msg = format!(
            r#"{{"event":"monitor_pending","ida":{},"external_id":"{}","masked_pan":"{}"}}"#,
            monitor.ida, monitor.external_id, monitor.masked_pan
        );
        if let Err(e) = ws_tx.send(msg) {
            warn!(ida = monitor.ida, ?e, "No WebSocket subscribers to notify");
        }

        info!(ida = monitor.ida, external_id = %monitor.external_id, "Scheduling fetch for account");
        if let Err(e) =
            fetch_and_persist_account(&monitor, req_last_taken, req_updated_at, &pool).await
        {
            error!(ida = monitor.ida, ?e, "Account fetch and persist failed");
        }

        if let Err(e) = update_monitor_status(
            monitor.ida,
            monitor.external_id.clone(),
            AccountMonitorStatus::Updated,
            &pool,
        )
        .await
        {
            error!(
                ida = monitor.ida,
                external_id = monitor.external_id.clone(),
                ?e,
                "Monitor status updated failed"
            );
        }

        let msg = format!(
            r#"{{"event":"monitor_updated","ida":{},"external_id":"{}","masked_pan":"{}"}}"#,
            monitor.ida, monitor.external_id, monitor.masked_pan
        );
        if let Err(e) = ws_tx.send(msg) {
            warn!(ida = monitor.ida, ?e, "No WebSocket subscribers to notify");
        }
    }

    let msg = format!(r#"{{"event":"all_monitors_updated"}}"#,);
    if let Err(e) = ws_tx.send(msg) {
        warn!(?e, "No WebSocket subscribers to notify");
    }
}

/// Compute the missing time ranges to fetch from Monobank.
///
/// Rules (both columns are compared against the *request* window):
///   - Part A (left gap):  `from < range_start` → missing range is `[from, range_start]`
///   - Part B (right gap): `to   > range_end`   → missing range is `[range_end, to]`
///
/// If either DB column is NULL the corresponding gap extends to the request boundary:
///   - `range_start` NULL → Part A is `[from, to]` (no left anchor, fetch everything)
///   - `range_end`   NULL → Part B is `[from, to]` (no right anchor, fetch everything)
///
/// Overlapping ranges (e.g. when gaps touch in the middle) are merged before returning.
fn compute_missing_ranges(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    range_end: Option<DateTime<Utc>>,
    range_start: Option<DateTime<Utc>>,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    if from >= to {
        return vec![];
    }

    // No markers at all – fetch the whole window.
    if range_end.is_none() && range_start.is_none() {
        return vec![(from, to)];
    }

    let mut ranges: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();

    // Part A: left gap — from < range_start
    match range_start {
        Some(rs) if from < rs => {
            // clamp right end so we never exceed `to`
            let end_a = rs.min(to);
            ranges.push((from, end_a));
        }
        None => {
            // No left anchor: treat whole window as potentially missing on this side.
            ranges.push((from, to));
        }
        _ => {} // from >= range_start — no left gap
    }

    // Part B: right gap — to > range_end
    match range_end {
        Some(re) if to > re => {
            // clamp left end so we never go before `from`
            let start_b = re.max(from);
            ranges.push((start_b, to));
        }
        None => {
            // No right anchor: treat whole window as potentially missing on this side.
            ranges.push((from, to));
        }
        _ => {} // to <= range_end — no right gap
    }

    // Merge overlapping / adjacent ranges.
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_by_key(|(s, _)| *s);
    let mut merged: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
    let mut cur = ranges[0];
    for &(s, e) in &ranges[1..] {
        if s <= cur.1 {
            if e > cur.1 {
                cur.1 = e;
            }
        } else {
            merged.push(cur);
            cur = (s, e);
        }
    }
    merged.push(cur);
    merged
}

/// split range into chunks of at most max_seconds seconds
fn split_into_chunks(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    max_seconds: i64,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut chunks = Vec::new();
    if start >= end {
        return chunks;
    }
    let mut cur = start;
    while cur < end {
        let next = (cur + chrono::Duration::seconds(max_seconds)).min(end);
        chunks.push((cur, next));
        cur = next;
    }
    chunks
}

#[instrument(skip(monitor, pool))]
async fn fetch_and_persist_account(
    monitor: &AccountMonitor,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    pool: &SqlitePool,
) -> Result<(), AppError> {
    use reqwest::StatusCode as ReqStatus;

    let external_id = monitor.external_id.clone();
    info!(ida = monitor.ida, external_id = %&external_id, from = %from, to = %to, "Fetching account transactions");

    // compute ranges
    let ranges = compute_missing_ranges(from, to, monitor.range_end, monitor.range_start);
    debug!(ida = monitor.ida, ranges = ?ranges, "Computed missing ranges");
    if ranges.is_empty() {
        // still update range_end to now
        repository::update_monitor_timestamps(monitor.ida, &external_id, Utc::now(), None, pool)
            .await?;
        info!(ida = monitor.ida, "No missing ranges, updated timestamps");
        return Ok(());
    }

    // get token
    let token = repository::get_token_by_ida(monitor.ida, pool)
        .await
        .map_err(|e| {
            error!(ida = monitor.ida, ?e, "Failed to get token");
            AppError::Internal("failed to retrieve token".into())
        })?;

    let client = reqwest::Client::new();
    const MAX_SECONDS: i64 = 2_682_000; // 31 days + 1 hour

    for (rstart, rend) in ranges {
        let chunks = split_into_chunks(rstart, rend, MAX_SECONDS);
        debug!(ida = monitor.ida, chunks = ?chunks, "Split into request chunks");
        for (cstart, cend) in chunks {
            // collect transactions to insert
            let mut transactions: Vec<BankTransaction> = Vec::new();
            // Monobank expects unix seconds
            let from_ts = cstart.timestamp();
            let to_ts = cend.timestamp();
            let url = format!(
                "https://api.monobank.ua/personal/statement/{}/{}/{}",
                monitor.external_id.clone(),
                from_ts,
                to_ts
            );

            // if fail retry
            let mut attempts = 0u8;
            loop {
                // respect rate limit: wait 61s before next request for same account
                info!(ida = monitor.ida, "Sleeping 60s to respect rate limits");
                sleep(Duration::from_secs(61)).await;

                attempts += 1;
                debug!(ida = monitor.ida, url = %url, attempt = attempts, "Requesting Monobank statement");
                let resp = client
                    .get(&url)
                    .header("X-Token", &token)
                    .header(USER_AGENT, "Local mimir")
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        debug!(ida = monitor.ida, status = %r.status(), "Successful response");
                        let txs = r.json::<Vec<MonobankTransaction>>().await.map_err(|e| {
                            error!(ida = monitor.ida, ?e, "Failed to parse Monobank response");
                            AppError::Internal("failed to parse upstream response".into())
                        })?;

                        debug!(
                            ida = monitor.ida,
                            tx_count = txs.len(),
                            "Parsed transactions"
                        );

                        for tx in txs {
                            let transaction_time = DateTime::<Utc>::from_timestamp(tx.time, 0)
                                .unwrap_or_else(Utc::now);

                            let nb = BankTransaction {
                                idt: tx.id,
                                external_id: monitor.external_id.clone(),
                                ida: monitor.ida,
                                amount: tx.amount,
                                currency_code: tx.currency_code,
                                description: tx.description,
                                mcc: tx.mcc,
                                hold: tx.hold,
                                transaction_time,
                                receipt_id: tx.receipt_id,
                                balance: Some(tx.balance),
                            };
                            debug!(ida = monitor.ida, bill_id = %nb.idt, amount = nb.amount, "Prepared NewBill");
                            transactions.push(nb);
                        }

                        break;
                    }
                    Ok(r) if r.status() == ReqStatus::TOO_MANY_REQUESTS => {
                        warn!(ida = monitor.ida, "Rate limited by Monobank, retrying");
                        if attempts >= 3 {
                            error!(ida = monitor.ida, "Too many requests, giving up");
                            break;
                        }
                        continue;
                    }
                    Ok(r) => {
                        error!(ida = monitor.ida, status = %r.status(), "Unexpected status from Monobank");
                        break;
                    }
                    Err(e) => {
                        warn!(
                            ida = monitor.ida,
                            ?e,
                            attempt = attempts,
                            "HTTP request error, will retry"
                        );
                        if attempts >= 3 {
                            error!(ida = monitor.ida, ?e, "Request failed after retries");
                            break;
                        }
                        // backoff
                        continue;
                    }
                }
            }

            transaction::repository::insert_transactions(&transactions, pool)
                .await
                .map_err(|e| {
                    error!(ida = monitor.ida, ?e, "Failed to insert transactions");
                    AppError::Internal("failed to persist transactions".into())
                })?;

            if !transactions.is_empty() {
                persist_sim_tags(monitor, &transactions, pool).await?;
            }
        }
    }

    let new_range_end = match get_highest_transaction_time(&monitor.external_id, pool).await {
        Ok(max_tx) => {
            // DB has transactions — use the most recent one
            max_tx
        }
        Err(_) => {
            // No transactions in DB — pick the bigger of monitor.range_end and `to`
            match monitor.range_end {
                Some(current) if current > to => current,
                _ => Utc::now(),
            }
        }
    };

    let mut maybe_rs: Option<DateTime<Utc>> = None;
    if let Some(rs) = monitor.range_start {
        if from < rs {
            maybe_rs = Some(from);
        }
    } else {
        maybe_rs = Some(from);
    }

    repository::update_monitor_timestamps(
        monitor.ida,
        &monitor.external_id,
        new_range_end,
        maybe_rs,
        pool,
    )
    .await
    .map_err(|e| {
        error!(ida = monitor.ida, ?e, "Failed to update monitor timestamps");
        AppError::Internal("failed to update monitor timestamps".into())
    })?;

    info!(
        ida = monitor.ida,
        "fetch_and_persist_account completed successfully"
    );

    Ok(())
}

async fn persist_sim_tags(
    monitor: &AccountMonitor,
    transactions: &[BankTransaction],
    pool: &SqlitePool,
) -> Result<(), AppError> {
    let mut grouped: HashMap<(u32, String), Vec<String>> = HashMap::new();

    for tx in transactions.iter() {
        if let (Some(mcc), Some(description_ref)) = (tx.mcc, tx.description.as_ref()) {
            let key = (mcc, description_ref.clone());
            grouped.entry(key).or_default().push(tx.idt.clone());
        }
    }
    // collect (mcc, description) pairs from the map keys
    let keys: Vec<(u32, String)> = grouped.keys().cloned().collect();
    let tags = transaction::repository::get_similar_transaction_tags(&keys, pool)
        .await
        .map_err(|e| {
            error!(ida = monitor.ida, ?e, "failed to fetch tags");
            AppError::Internal("failed to fetch tags".into())
        })?;

    // add transactions tags
    let mut tags_insert_list = vec![];

    for (key, value) in tags.into_iter() {
        if let Some(idt_list) = grouped.get(&key) {
            // convert HashSet<TransactionTag> -> Vec<TransactionTag>
            let tags_vec: Vec<TransactionTag> = value.into_iter().collect();
            for tr in idt_list.iter() {
                let tag = BankTransactionTag {
                    idt: tr.clone(),
                    tags: tags_vec.clone(),
                };
                tags_insert_list.push(tag);
            }
        }
    }
    let _ = add_transaction_tags(&tags_insert_list, pool).await;
    Ok(())
}

/// fetches all user banks and sniffs their accounts.
async fn sniff_accounts(pool: &SqlitePool) {
    let accounts = repository::find_all(pool).await;
    let mono_accounts = accounts.into_iter().filter(|a| a.kind == AccountKind::Mono);

    sniff_monobank_accounts(mono_accounts, pool).await;
}

/// Calls the Monobank API concurrently for each account, then persists results.
async fn sniff_monobank_accounts(accounts: impl Iterator<Item = Account>, pool: &SqlitePool) {
    const MONO_CLIENT_INFO_URL: &str = "https://api.monobank.ua/personal/client-info";

    let mut set: JoinSet<Result<Vec<MonoAccount>, String>> = JoinSet::new();

    for account in accounts {
        let token = match repository::get_token_by_ida(account.ida, pool).await {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to fetch token for ida={}: {}", account.ida, e);
                continue;
            }
        };
        let ida = account.ida;

        set.spawn(async move {
            let client = reqwest::Client::new();
            let info = client
                .get(MONO_CLIENT_INFO_URL)
                .header("X-Token", &token)
                .header(USER_AGENT, "Local mimir")
                .send()
                .await
                .map_err(|e| {
                    error!("Monobank request error for ida={}: {}", ida, e);
                    "upstream request failed".to_string()
                })?
                .text()
                .await
                .map_err(|e| {
                    error!("Monobank read body error for ida={}: {}", ida, e);
                    "upstream response read failed".to_string()
                })
                .and_then(|body| {
                    serde_json::from_str::<MonoClientInfo>(&body).map_err(|e| {
                        error!(
                            "Monobank deserialize error for ida={}: {}\nBody: {}",
                            ida, e, body
                        );
                        "upstream response parse failed".to_string()
                    })
                })?;

            let mut mono_accounts = info.accounts;
            for a in mono_accounts.iter_mut() {
                a.ida = ida;
            }
            Ok(mono_accounts)
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(mono_accounts)) => {
                if let Err(e) = repository::insert_mono_accounts_monitor(&mono_accounts, pool).await
                {
                    error!("Failed to persist account monitors: {}", e);
                }
            }
            Ok(Err(msg)) => error!("Monobank task error: {}", msg),
            Err(e) => error!("Task panicked: {}", e),
        }
    }
}

/// Returns all account monitor rows
async fn get_monitors(pool: &SqlitePool) -> Vec<AccountMonitor> {
    let mut monitors = repository::fetch_all_monitors(&pool).await;
    monitors.sort_by_key(|m| m.balance);
    monitors.reverse();
    monitors
}
