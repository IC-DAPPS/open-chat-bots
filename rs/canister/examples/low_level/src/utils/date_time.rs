use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

pub fn format_timestamp(timestamp_ns: u64, timezone: &str) -> Result<String, String> {
    // Convert to seconds and nanoseconds
    let seconds = (timestamp_ns / 1_000_000_000) as i64;
    let nanoseconds = (timestamp_ns % 1_000_000_000) as u32;

    // Parse the timezone string
    let tz = Tz::from_str(timezone).map_err(|e| format!("Invalid timezone: {}", e))?;

    // Create a UTC DateTime from the timestamp
    let utc_time = Utc
        .timestamp_opt(seconds, nanoseconds)
        .single()
        .ok_or_else(|| "Invalid timestamp".to_string())?;

    // Convert to the specified timezone
    let local_time: DateTime<Tz> = utc_time.with_timezone(&tz);

    // Format the datetime (customize this format as needed)
    Ok(local_time.format("%Y-%m-%d %H:%M:%S %Z").to_string())
}
