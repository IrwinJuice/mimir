use super::model::{
    Account, AccountKind, AccountMonitor, AccountMonitorStatus, CreateAccount, MonoAccount,
    MonoClientInfo, StatQueryParams,
};
use super::repository;
use crate::account::repository::update_monitor_status;
use crate::error::AppError;
use crate::transaction;
use crate::transaction::{BankTransaction, MonobankTransaction};
use crate::utils::datetime::{DateTimeUtc, TimestampRange};
use crate::ws_handler::WsTx;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::USER_AGENT;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, instrument, warn};

/// POST /bills/api/users/:idu/accounts
#[instrument(skip(pool, payload))]
pub async fn add_account(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateAccount>,
) -> Result<(StatusCode, Json<Account>), AppError> {
    debug!(idu = payload.idu, "Adding account");
    let account = repository::insert(payload.idu, payload.kind, payload.token, &pool).await?;
    info!(ida = account.ida, "Account created");
    Ok((StatusCode::CREATED, Json(account)))
}

/// GET /bills/api/users/:idu/accounts
#[instrument(skip(pool))]
pub async fn get_accounts_by_idu(
    Path(idu): Path<u32>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<Account>>, AppError> {
    debug!(idu, "Fetching accounts for user");
    let accounts = repository::find_by_idu(idu, &pool).await?;
    debug!(count = accounts.len(), "Found accounts");
    Ok(Json(accounts))
}

/// GET /bills/api/users/{idu}/monitors
#[instrument(skip(pool))]
pub async fn get_accounts_monitors(
    Path(idu): Path<u32>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<AccountMonitor>>, AppError> {
    debug!(idu, "Fetching monitors for user");
    let monitors = repository::fetch_all_monitors_by_idu(idu, &pool).await?;
    debug!(count = monitors.len(), "Found monitors");
    Ok(Json(monitors))
}

/// PUT /bills/api/users/:idu/accounts/stat
#[instrument(skip(pool, params, ws_tx))]
pub async fn update_accounts_stat(
    Query(params): Query<StatQueryParams>,
    Path(idu): Path<u32>,
    State(pool): State<SqlitePool>,
    State(ws_tx): State<WsTx>,
) -> Result<(StatusCode, Json<Vec<AccountMonitor>>), AppError> {
    info!(idu, from = %params.from, to = %params.to, "Updating account stat");
    //
    // // Return the user-provided JSON as typed response (constructed manually so we don't touch comments)
    // let monitors_for_response: Vec<AccountMonitor> = vec![
    //     AccountMonitor {
    //         ida: 1,
    //         external_id: "JQzeEVplrSlv9f7sh9hFLw".to_string(),
    //         currency_code: 980,
    //         balance: 1664,
    //         credit_limit: 0,
    //         iban: "UA563220010000026202351168494".to_string(),
    //         masked_pan: "444111******5014".to_string(),
    //         kind: AccountKind::Mono,
    //         updated_at: None,
    //         last_taken_date: None,
    //         status: AccountMonitorStatus::Pending,
    //     },
    //     AccountMonitor {
    //         ida: 1,
    //         external_id: "j2ft-PJ0LDors9INaNMcgw".to_string(),
    //         currency_code: 840,
    //         balance: 9424,
    //         credit_limit: 0,
    //         iban: "UA773220010000026206330587199".to_string(),
    //         masked_pan: "444111******4069".to_string(),
    //         kind: AccountKind::Mono,
    //         updated_at: None,
    //         last_taken_date: None,
    //         status: AccountMonitorStatus::Never,
    //     },
    //     AccountMonitor {
    //         ida: 1,
    //         external_id: "Vb2IecNJleJpaf68itjujQ".to_string(),
    //         currency_code: 980,
    //         balance: 592970,
    //         credit_limit: 100000,
    //         iban: "UA743220010000026201303310150".to_string(),
    //         masked_pan: "444111******3308".to_string(),
    //         kind: AccountKind::Mono,
    //         updated_at: None,
    //         last_taken_date: None,
    //         status: AccountMonitorStatus::Never,
    //     },
    //     AccountMonitor {
    //         ida: 1,
    //         external_id: "9e0PiuTyuEix2kVUbq4sbg".to_string(),
    //         currency_code: 980,
    //         balance: 30911932,
    //         credit_limit: 0,
    //         iban: "UA213220010000026204307220975".to_string(),
    //         masked_pan: "444111******4488".to_string(),
    //         kind: AccountKind::Mono,
    //         updated_at: None,
    //         last_taken_date: None,
    //         status: AccountMonitorStatus::Never,
    //     },
    //     AccountMonitor {
    //         ida: 1,
    //         external_id: "dy8oAcqW4ngR5Wy_SPO-kA".to_string(),
    //         currency_code: 980,
    //         balance: 0,
    //         credit_limit: 0,
    //         iban: "UA373220010000026200320095051".to_string(),
    //         masked_pan: "444111******1497".to_string(),
    //         kind: AccountKind::Mono,
    //         updated_at: None,
    //         last_taken_date: None,
    //         status: AccountMonitorStatus::Never,
    //     },
    // ];


    sniff_accounts(&pool).await;
    let monitors = get_account_monitors_by_idu(idu, &pool).await?;
    debug!("monitors_get_by_idu {:?}", &monitors);
    let monitors_for_response = monitors.clone();

    // Query params mapping: `from` = last_taken_date, `to` = updated_at
    let DateTimeUtc(req_last_taken) = params.from.clone().try_into().map_err(|e| {
        error!(idu = idu, from = %params.from, "Invalid 'from' datetime: {}", e);
        AppError::BadRequest(e)
    })?;

    let DateTimeUtc(req_updated_at) = params.to.clone().try_into().map_err(|e| {
        error!(idu = idu, to = %params.to, "Invalid 'to' datetime: {}", e);
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
    for monitor in monitors {
        let msg = format!(
            r#"{{"event":"monitor_pending","ida":{},"external_id":"{}","masked_pan":"{}"}}"#,
            monitor.ida, monitor.external_id, monitor.masked_pan
        );
        if let Err(e) = ws_tx.send(msg) {
            warn!(ida = monitor.ida, ?e, "No WebSocket subscribers to notify");
        }

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
                "Monitor status update  failed"
            );
        }

        // respect rate limit: wait 61s before next request for same account
        // we sniff accounts before this fn, need to wait
        info!(ida = monitor.ida, "Sleeping 60s to respect rate limits");
        sleep(Duration::from_secs(61)).await;

        info!(ida = monitor.ida, external_id = %monitor.external_id, "Scheduling fetch for account");
        if let Err(e) = fetch_and_persist_account(&monitor, req_last_taken, req_updated_at, &pool).await {
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
    let ranges = compute_missing_ranges(from, to, monitor.updated_at, monitor.last_taken_date);
    debug!(ida = monitor.ida, ranges = ?ranges, "Computed missing ranges");
    if ranges.is_empty() {
        // still update updated_at to now
        repository::update_monitor_timestamps(monitor.ida, external_id, Utc::now(), None, pool)
            .await?;
        info!(ida = monitor.ida, "No missing ranges, updated timestamps");
        return Ok(());
    }

    // collect bills to insert
    let mut all_bills: Vec<BankTransaction> = Vec::new();

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
                attempts += 1;
                debug!(ida = monitor.ida, url = %url, attempt = attempts, "Requesting Monobank statement");
                let resp = client
                    .get(&url)
                    .header("X-Token", &token)
                    .header(USER_AGENT, "Local bills")
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
                                id: tx.id,
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
                            debug!(ida = monitor.ida, bill_id = %nb.id, amount = nb.amount, "Prepared NewBill");
                            all_bills.push(nb);
                        }

                        // respect rate limit: wait 61s before next request for same account
                        info!(ida = monitor.ida, "Sleeping 60s to respect rate limits");
                        sleep(Duration::from_secs(61)).await;
                        break;
                    }
                    Ok(r) if r.status() == ReqStatus::TOO_MANY_REQUESTS => {
                        warn!(ida = monitor.ida, "Rate limited by Monobank, retrying");
                        if attempts >= 3 {
                            error!(ida = monitor.ida, "Too many requests, giving up");
                            break;
                        }
                        sleep(Duration::from_secs(61)).await;
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
                        sleep(Duration::from_millis(500 * (attempts as u64))).await;
                        continue;
                    }
                }
            }
        }
    }

    // persist bills
    info!(
        ida = monitor.ida,
        bill_count = all_bills.len(),
        "Persisting bills"
    );
    transaction::repository::insert_transactions(&all_bills, pool)
        .await
        .map_err(|e| {
            error!(ida = monitor.ida, ?e, "Failed to insert bills");
            AppError::Internal("failed to persist bills".into())
        })?;

    // update monitor timestamps per rules: updated_at = now, last_taken_date = from if from < last_taken_date
    let now = Utc::now();
    let mut maybe_lt: Option<DateTime<Utc>> = None;
    if let Some(ltd) = monitor.last_taken_date {
        if from < ltd {
            maybe_lt = Some(from);
        }
    } else {
        maybe_lt = Some(from);
    }

    repository::update_monitor_timestamps(
        monitor.ida,
        monitor.external_id.clone(),
        now,
        maybe_lt,
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
            let info = client
                .get(MONO_CLIENT_INFO_URL)
                .header("X-Token", &token)
                .header(USER_AGENT, "Local bills")
                .send()
                .await
                .map_err(|e| {
                    error!("Monobank request error for ida={}: {}", ida, e);
                    "upstream request failed".to_string()
                })?
                .json::<MonoClientInfo>()
                .await
                .map_err(|e| {
                    error!("Monobank deserialize error for ida={}: {}", ida, e);
                    "upstream response parse failed".to_string()
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
async fn get_account_monitors_by_idu(
    idu: u32,
    pool: &SqlitePool,
) -> Result<Vec<AccountMonitor>, sqlx::Error> {
    repository::fetch_all_monitors_by_idu(idu, pool).await
}
