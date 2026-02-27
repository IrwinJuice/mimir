use chrono::{DateTime, NaiveDateTime, Utc};

/// Range in Unix Timestamp
pub struct TimestampRange {
    pub from: i64,
    pub to: i64,
}

pub struct DateTimeUtc(pub DateTime<Utc>);

impl TryFrom<i64> for DateTimeUtc {
    type Error = String;

    fn try_from(ts: i64) -> Result<Self, Self::Error> {
        DateTime::from_timestamp(ts, 0)
            .map(|n| DateTimeUtc(n))
            .ok_or_else(|| format!("Invalid Unix timestamp: {}", ts))
    }
}

impl TryFrom<String> for DateTimeUtc {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        // Try RFC3339 / ISO 8601 first (e.g. 2026-02-25T10:00:00Z or with offset)
        if let Ok(dt) = DateTime::parse_from_rfc3339(&value) {
            return Ok(DateTimeUtc(dt.with_timezone(&Utc)));
        }

        // Try the original format with explicit timezone: "YYYY-MM-DD HH:MM:SS ±HHMM"
        if let Ok(dt) = DateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S %z") {
            return Ok(DateTimeUtc(dt.with_timezone(&Utc)));
        }

        // Try naive datetime without timezone and assume UTC: "YYYY-MM-DD HH:MM:SS"
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S") {
            return Ok(DateTimeUtc(DateTime::<Utc>::from_utc(naive, Utc)));
        }

        Err(format!(
            "Invalid datetime '{}': supported formats are RFC3339 (e.g. 2026-02-25T10:00:00Z), 'YYYY-MM-DD HH:MM:SS ±HHMM', or 'YYYY-MM-DD HH:MM:SS'",
            value
        ))
    }
}
