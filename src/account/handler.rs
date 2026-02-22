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
use sqlx::SqlitePool;
use std::collections::HashMap;
use tokio::task::JoinSet;
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

    let DateTimeUtc(form) = params.from.try_into()?;
    let DateTimeUtc(to) = params.to.try_into()?;





    Ok((StatusCode::OK, Json(monitors)))
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
                if let Err(e) = repository::insert_accounts_monitor(&mono_accounts, pool).await {
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
