use std::collections::HashMap;
use axum::Json;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tracing::{instrument};
use crate::error::AppError;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Localized {
    pub uk: Option<String>,
    pub en: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Group {
    #[serde(rename = "type")]
    pub r#type: String,
    pub description: Localized,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct MccEntry {
    pub mcc: String,
    pub group: Group,
    #[serde(rename = "fullDescription")]
    pub full_description: Localized,
    #[serde(rename = "shortDescription")]
    pub short_description: Localized,
}

static MCC_JSON: &str = include_str!("../mcc/mcc.json");

static MCC_MAP: Lazy<HashMap<String, MccEntry>> = Lazy::new(|| {
    let items: Vec<MccEntry> = serde_json::from_str(MCC_JSON).expect("Failed to parse mcc.json");
    let mut map = HashMap::with_capacity(items.len());
    for item in items {
        map.insert(item.mcc.clone(), item);
    }
    map
});

/// Normalize MCC code: trim and left-pad with zeros to 4 chars if numeric.
fn normalize(code: &str) -> Option<String> {
    let s = code.trim();
    if s.is_empty() {
        return None;
    }
    // If code contains only digits, left-pad to 4 digits
    if s.chars().all(|c| c.is_ascii_digit()) {
        let n = s.len();
        if n > 4 {
            return None; // invalid length
        }
        return Some(format!("{:0>4}", s));
    }
    // otherwise return as-is
    Some(s.to_string())
}

/// Lookup MCC by code. Accepts codes like "742" or "0742".
pub fn lookup_mcc(code: &str) -> Option<&'static MccEntry> {
    let key = normalize(code)?;
    MCC_MAP.get(&key)
}

#[instrument]
pub async fn get_all_mcc(
) -> Result<Json<&'static HashMap<String, MccEntry>>, AppError> {
    // Return a reference to the static map; the map and its values live for 'static
    Ok(Json(&*MCC_MAP))
}
