use crate::error::AppError;
use crate::mcc_data::{lookup_mcc, MccEntry};
use crate::transaction::model::{
    BankTransactionDTO, BankTransactionFilter, BankTransactionTag, MagicBankTransactionTag,
    TransactionTag,
};
use crate::transaction::repository;
use axum::extract::{State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::Json;
use iso_currency::Currency;
use rust_xlsxwriter::Workbook;
use sqlx::SqlitePool;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, instrument};

#[instrument(skip(pool))]
pub async fn get_mcc(State(pool): State<SqlitePool>) -> Result<Json<Vec<MccEntry>>, AppError> {
    debug!("Fetching mcc");

    let mcc_list = repository::get_mcc(&pool).await?;
    let mut result: Vec<MccEntry> = vec![];

    for mcc in mcc_list.iter() {
        match lookup_mcc(&mcc.to_string()) {
            None => {
                error!(mcc, "Merchant Category Code not found")
            }
            Some(m) => result.push(m.clone()),
        }
    }

    Ok(Json(result))
}

async fn fetch_transactions(
    params: BankTransactionFilter,
    pool: &SqlitePool,
) -> Result<Vec<BankTransactionDTO>, AppError> {
    debug!(?params, "Fetching transactions");

    let t_list = repository::get_transactions(params, &pool).await?;

    // Transform the list: populate `currency` and `mcc_description` on each DTO
    let x: Vec<BankTransactionDTO> = t_list
        .into_iter()
        .map(|mut t| {
            // currency: map numeric code to ISO code string
            t.currency =
                Currency::from_numeric(t.currency_code as u16).map(|c| c.code().to_string());

            t.mcc_description = t.mcc.and_then(|mcc| {
                lookup_mcc(&mcc.to_string()).and_then(|entry| {
                    entry
                        .short_description
                        .uk
                        .clone()
                        .or(entry.full_description.uk.clone())
                        .or(entry.short_description.en.clone())
                        .or(entry.full_description.en.clone())
                })
            });

            t
        })
        .collect();
    Ok(x)
}

#[instrument(skip(pool))]
pub async fn get_transactions(
    State(pool): State<SqlitePool>,
    Json(params): Json<BankTransactionFilter>,
) -> Result<Json<Vec<BankTransactionDTO>>, AppError> {
    Ok(Json(fetch_transactions(params, &pool).await?))
}

#[instrument(skip(pool))]
pub async fn download_csv(
    State(pool): State<SqlitePool>,
    Json(params): Json<BankTransactionFilter>,
) -> Result<impl IntoResponse, AppError> {
    let data = fetch_transactions(params, &pool).await?;

    // Write records into an in-memory byte buffer
    let mut wtr = csv::Writer::from_writer(vec![]);
    // Write header manually so we can include the tags column
    wtr.write_record(&[
        "idt",
        "external_id",
        "ida",
        "amount",
        "currency_code",
        "description",
        "mcc",
        "hold",
        "transaction_time",
        "receipt_id",
        "balance",
        "masked_pan",
        "mcc_description",
        "currency",
        "tags",
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?;
    for t in &data {
        let tags_str = t
            .tags
            .iter()
            .map(|tag| tag.tag.as_str())
            .collect::<Vec<_>>()
            .join(",");
        wtr.write_record(&[
            t.idt.as_str(),
            t.external_id.as_str(),
            &t.ida.to_string(),
            &t.amount.to_string(),
            &t.currency_code.to_string(),
            t.description.as_deref().unwrap_or(""),
            &t.mcc.map(|v| v.to_string()).unwrap_or_default(),
            &t.hold.map(|v| v.to_string()).unwrap_or_default(),
            &t.transaction_time.to_rfc3339(),
            t.receipt_id.as_deref().unwrap_or(""),
            &t.balance.map(|v| v.to_string()).unwrap_or_default(),
            t.masked_pan.as_deref().unwrap_or(""),
            t.mcc_description.as_deref().unwrap_or(""),
            t.currency.as_deref().unwrap_or(""),
            &tags_str,
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let stream = ReaderStream::new(std::io::Cursor::new(bytes));
    let body = axum::body::Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/csv; charset=utf-8".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"transactions.csv\"".parse().unwrap(),
    );

    Ok((headers, body).into_response())
}

#[instrument(skip(pool))]
pub async fn download_json(
    State(pool): State<SqlitePool>,
    Json(params): Json<BankTransactionFilter>,
) -> Result<impl IntoResponse, AppError> {
    let data = fetch_transactions(params, &pool).await?;

    let bytes = serde_json::to_vec_pretty(&data).map_err(|e| AppError::Internal(e.to_string()))?;

    let stream = ReaderStream::new(std::io::Cursor::new(bytes));
    let body = axum::body::Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/json; charset=utf-8".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"transactions.json\""
            .parse()
            .unwrap(),
    );

    Ok((headers, body).into_response())
}

#[instrument(skip(pool))]
pub async fn download_xlsx(
    State(pool): State<SqlitePool>,
    Json(params): Json<BankTransactionFilter>,
) -> Result<impl IntoResponse, AppError> {
    let data = fetch_transactions(params, &pool).await?;

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    // Header row
    let headers = [
        "idt",
        "external_id",
        "ida",
        "amount",
        "currency_code",
        "description",
        "mcc",
        "hold",
        "transaction_time",
        "receipt_id",
        "balance",
        "masked_pan",
        "mcc_description",
        "currency",
        "tags",
    ];
    for (col, h) in headers.iter().enumerate() {
        sheet
            .write_string(0, col as u16, *h)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    // Data rows
    for (row, t) in data.iter().enumerate() {
        let row = (row + 1) as u32;
        sheet
            .write_string(row, 0, &t.idt)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_string(row, 1, &t.external_id)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_number(row, 2, t.ida as f64)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_number(row, 3, t.amount as f64)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_number(row, 4, t.currency_code as f64)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_string(row, 5, t.description.as_deref().unwrap_or(""))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(mcc) = t.mcc {
            sheet
                .write_number(row, 6, mcc as f64)
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }
        if let Some(hold) = t.hold {
            sheet
                .write_boolean(row, 7, hold)
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }
        sheet
            .write_string(row, 8, &t.transaction_time.to_rfc3339())
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_string(row, 9, t.receipt_id.as_deref().unwrap_or(""))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(balance) = t.balance {
            sheet
                .write_number(row, 10, balance as f64)
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }
        sheet
            .write_string(row, 11, t.masked_pan.as_deref().unwrap_or(""))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_string(row, 12, t.mcc_description.as_deref().unwrap_or(""))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sheet
            .write_string(row, 13, t.currency.as_deref().unwrap_or(""))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let tags_str = t
            .tags
            .iter()
            .map(|tag| tag.tag.as_str())
            .collect::<Vec<_>>()
            .join(",");
        sheet
            .write_string(row, 14, &tags_str)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    let bytes = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let stream = ReaderStream::new(std::io::Cursor::new(bytes));
    let body = axum::body::Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"transactions.xlsx\""
            .parse()
            .unwrap(),
    );

    Ok((headers, body).into_response())
}

#[instrument(skip(pool))]
pub async fn add_transactions_tags(
    State(pool): State<SqlitePool>,
    Json(tags): Json<Vec<BankTransactionTag>>,
) -> Result<Json<Vec<BankTransactionTag>>, AppError> {
    debug!(?tags, "Add transaction tags");
    Ok(Json(repository::add_transaction_tags(&tags, &pool).await?))
}

#[instrument(skip(pool))]
pub async fn delete_transactions_tags(
    State(pool): State<SqlitePool>,
    Json(tags): Json<Vec<BankTransactionTag>>,
) -> Result<Json<Vec<BankTransactionTag>>, AppError> {
    debug!(?tags, "Delete transaction tags");
    Ok(Json(
        repository::delete_transaction_tags(&tags, &pool).await?,
    ))
}

#[instrument(skip(pool))]
pub async fn get_transaction_tags(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<TransactionTag>>, AppError> {
    debug!("Fetching transaction tags");
    Ok(Json(repository::get_transaction_tags(&pool).await?))
}

#[instrument(skip(pool))]
pub async fn get_transaction_tags_names(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<String>>, AppError> {
    debug!("Fetching transaction tags");
    Ok(Json(repository::get_transaction_tags_names(&pool).await?))
}
