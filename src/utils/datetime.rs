use chrono::{DateTime, Utc};

pub struct DateTimeUtc(pub DateTime<Utc>);

impl TryFrom<String> for DateTimeUtc {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        DateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S %z")
            .map(|dt| DateTimeUtc(dt.to_utc()))
            .map_err(|e| format!("Invalid datetime '{}': {}", value, e))
    }
}
