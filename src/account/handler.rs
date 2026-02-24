use super::model::{
    Account, AccountKind, AccountMonitor, CreateAccount, MonoAccount, StatQueryParams,
};
use super::repository;
use crate::error::AppError;
use crate::utils::datetime::DateTimeUtc;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::USER_AGENT;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::error;

/// POST /bills/api/users/:idu/accounts
pub async fn add_account(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateAccount>,
) -> Result<(StatusCode, Json<Account>), AppError> {
    let account = repository::insert(payload.idu, payload.kind, payload.token, &pool).await?;
    Ok((StatusCode::CREATED, Json(account)))
}

/// GET /bills/api/users/:idu/accounts
pub async fn get_accounts_by_idu(
    Path(idu): Path<u32>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<Account>>, AppError> {
    let accounts = repository::find_by_idu(idu, &pool).await?;
    Ok(Json(accounts))
}

/// PATCH /bills/api/users/:idu/accounts/stat
pub async fn update_account_stat(
    Query(params): Query<StatQueryParams>,
    Path(idu): Path<u32>,
    State(pool): State<SqlitePool>,
) -> Result<(StatusCode, Json<Vec<AccountMonitor>>), AppError> {
    sniff_accounts(&pool).await;
    let monitors = get_account_monitors_by_idu(idu, &pool).await?;
    let monitors_for_response = monitors.clone();

    // Query params mapping: `from` = last_taken_date, `to` = updated_at
    let DateTimeUtc(req_last_taken) = params.from.try_into()?;
    let DateTimeUtc(req_updated_at) = params.to.try_into()?;

    let monobank: Vec<AccountMonitor> = monitors.iter().filter(|m| m.kind == AccountKind::Mono).cloned().collect();

    update_mono_accounts_stat(monobank, req_last_taken, req_updated_at, &pool).await;

    Ok((StatusCode::OK, Json(monitors_for_response)))
}

async fn update_mono_accounts_stat(monitors: Vec<AccountMonitor>, req_last_taken: DateTime<Utc>, req_updated_at: DateTime<Utc>, pool: &SqlitePool) {
    use std::sync::Arc;
    // concurrency limit for accounts
    let sem = Arc::new(Semaphore::new(4));
    let mut set: JoinSet<()> = JoinSet::new();

    for monitor in monitors {
        let sem = sem.clone();
        let permit = sem.acquire_owned().await.unwrap();
        let pool = pool.clone();
        let mon = monitor.clone();
        let f = req_last_taken.clone();
        let t = req_updated_at.clone();

        set.spawn(async move {
            let _p = permit; // keep permit until task ends
            if let Err(e) = fetch_and_persist_account(&mon, f, t, &pool).await {
                error!("Account ida={} failed: {:?}", mon.ida, e);
            }
        });
    }

    while let Some(_) = set.join_next().await {}
}

/// Compute the missing time ranges to fetch from Monobank.
///
/// Rules (both columns are compared against the *request* window):
///   - Part A (left gap):  `from < last_taken_date` → missing range is `[from, last_taken_date]`
///   - Part B (right gap): `to   > updated_at`      → missing range is `[updated_at, to]`
///
/// If either DB column is NULL the corresponding gap extends to the request boundary:
///   - `last_taken_date` NULL → Part A is `[from, to]` (no left anchor, fetch everything)
///   - `updated_at`      NULL → Part B is `[from, to]` (no right anchor, fetch everything)
///
/// Overlapping ranges (e.g. when gaps touch in the middle) are merged before returning.
fn compute_missing_ranges(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
    last_taken_date: Option<DateTime<Utc>>,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    if from >= to {
        return vec![];
    }

    // No markers at all – fetch the whole window.
    if updated_at.is_none() && last_taken_date.is_none() {
        return vec![(from, to)];
    }

    let mut ranges: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();

    // Part A: left gap — from < last_taken_date
    match last_taken_date {
        Some(ltd) if from < ltd => {
            // clamp right end so we never exceed `to`
            let end_a = ltd.min(to);
            ranges.push((from, end_a));
        }
        None => {
            // No left anchor: treat whole window as potentially missing on this side.
            ranges.push((from, to));
        }
        _ => {} // from >= last_taken_date — no left gap
    }

    // Part B: right gap — to > updated_at
    match updated_at {
        Some(ua) if to > ua => {
            // clamp left end so we never go before `from`
            let start_b = ua.max(from);
            ranges.push((start_b, to));
        }
        None => {
            // No right anchor: treat whole window as potentially missing on this side.
            ranges.push((from, to));
        }
        _ => {} // to <= updated_at — no right gap
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
            if e > cur.1 { cur.1 = e; }
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

async fn fetch_and_persist_account(
    monitor: &AccountMonitor,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    pool: &SqlitePool,
) -> Result<(), AppError> {
    use super::model::{MonobankTransaction, NewBill};
    use reqwest::StatusCode as ReqStatus;

    // compute ranges
    let ranges = compute_missing_ranges(from, to, monitor.updated_at, monitor.last_taken_date);
    if ranges.is_empty() {
        // still update updated_at to now
        repository::update_monitor_timestamps(monitor.ida, Utc::now(), None, pool).await?;
        return Ok(());
    }

    // collect bills to insert
    let mut all_bills: Vec<NewBill> = Vec::new();

    // get token
    let token = repository::get_token_by_ida(monitor.ida, pool).await.map_err(|e| {
        error!("Failed to get token for ida={}: {}", monitor.ida, e);
        AppError::Internal("failed to retrieve token".into())
    })?;

    let client = reqwest::Client::new();
    const MAX_SECONDS: i64 = 2_682_000; // 31 days + 1 hour

    for (rstart, rend) in ranges {
        let chunks = split_into_chunks(rstart, rend, MAX_SECONDS);
        for (cstart, cend) in chunks {
            // Monobank expects unix seconds
            let from_ts = cstart.timestamp();
            let to_ts = cend.timestamp();
            let url = format!("https://api.monobank.ua/personal/statement/{}/{}/{}/", monitor.external_id, from_ts, to_ts);

            let mut attempts = 0u8;
            loop {
                attempts += 1;
                let resp = client
                    .get(&url)
                    .header("X-Token", &token)
                    .header(USER_AGENT, "Local bills")
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        let txs = r.json::<Vec<MonobankTransaction>>().await.map_err(|e| {
                            error!("Failed to parse Monobank response ida={}: {}", monitor.ida, e);
                            AppError::Internal("failed to parse upstream response".into())
                        })?;

                        for tx in txs {
                            let transaction_time = DateTime::<Utc>::from_timestamp(tx.time, 0)
                                .unwrap_or_else(Utc::now);

                            let nb = NewBill {
                                id: tx.id,
                                ida: monitor.ida,
                                amount: tx.amount,
                                currency_code: tx.currency_code,
                                description: tx.description,
                                mcc: tx.mcc,
                                hold: tx.hold,
                                transaction_time,
                                receipt_id: tx.receipt_id,
                                balance: tx.balance,
                            };
                            all_bills.push(nb);
                        }

                        // respect rate limit: wait 60s before next request for same account
                        sleep(Duration::from_secs(60)).await;
                        break;
                    }
                    Ok(r) if r.status() == ReqStatus::TOO_MANY_REQUESTS => {
                        if attempts >= 3 {
                            error!("Too many requests for ida={}", monitor.ida);
                            break;
                        }
                        sleep(Duration::from_secs(60)).await;
                        continue;
                    }
                    Ok(r) => {
                        error!("Unexpected status {} for ida={}", r.status(), monitor.ida);
                        break;
                    }
                    Err(e) => {
                        if attempts >= 3 {
                            error!("Request failed for ida={} after {} attempts: {}", monitor.ida, attempts, e);
                            break;
                        }
                        // backoff
                        sleep(Duration::from_millis(500 * (attempts as u64))).await;
                        continue;
                    }
                }
            }
        }
    }

    // persist bills
    repository::insert_bills(&all_bills, pool).await.map_err(|e| {
        error!("Failed to insert bills for ida={}: {}", monitor.ida, e);
        AppError::Internal("failed to persist bills".into())
    })?;

    // update monitor timestamps per rules: updated_at = now, last_taken_date = from if from < last_taken_date
    let now = Utc::now();
    let mut maybe_lt: Option<DateTime<Utc>> = None;
    if let Some(ltd) = monitor.last_taken_date {
        if from > ltd {
            maybe_lt = Some(from);
        }
    }

    repository::update_monitor_timestamps(monitor.ida, now, maybe_lt, pool).await.map_err(|e| {
        error!("Failed to update monitor timestamps for ida={}: {}", monitor.ida, e);
        AppError::Internal("failed to update monitor timestamps".into())
    })?;

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
        let token = account.token.clone();
        let ida = account.ida;

        set.spawn(async move {
            let client = reqwest::Client::new();
            let mut mono_accounts = client
                .get(MONO_CLIENT_INFO_URL)
                .header("X-Token", &token)
                .header(USER_AGENT, "Local bills")
                .send()
                .await
                .map_err(|e| {
                    error!("Monobank request error for ida={}: {}", ida, e);
                    "upstream request failed".to_string()
                })?
                .json::<Vec<MonoAccount>>()
                .await
                .map_err(|e| {
                    error!("Monobank deserialize error for ida={}: {}", ida, e);
                    "upstream response parse failed".to_string()
                })?;

            for a in mono_accounts.iter_mut() {
                a.ida = ida;
            }
            Ok(mono_accounts)
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(mono_accounts)) => {
                if let Err(e) = repository::insert_mono_accounts_monitor(&mono_accounts, pool).await {
                    error!("Failed to persist account monitors: {}", e);
                }
            }
            Ok(Err(msg)) => error!("Monobank task error: {}", msg),
            Err(e) => error!("Task panicked: {}", e),
        }
    }
}

/// Returns all account monitor rows
async fn get_account_monitors_by_idu(
    idu: u32,
    pool: &SqlitePool,
) -> Result<Vec<AccountMonitor>, sqlx::Error> {
    repository::find_all_monitors_by_idu(idu, pool).await
}
