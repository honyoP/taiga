//! Natural language date parsing module
//!
//! Supports various date formats:
//! - ISO dates: "2026-01-25"
//! - Human dates: "Jan 25", "January 25 2026"
//! - Relative: "today", "tomorrow", "monday", "next friday"
//! - Offset: "in 3 days", "in 1 week"

use chrono::{Datelike, Days, Local, NaiveDate, Weekday};

use crate::error::{CoreError, Result};

/// Parse a date string into a NaiveDate
///
/// Supports multiple formats:
/// - ISO: "2026-01-25"
/// - Human: "Jan 25", "January 25", "Jan 25 2026"
/// - Relative: "today", "tomorrow"
/// - Weekdays: "monday", "tuesday", etc. (next occurrence)
/// - Prefixed: "next monday", "next friday"
/// - Offset: "in 3 days", "in 1 week", "in 2 weeks"
pub fn parse_date(input: &str) -> Result<NaiveDate> {
    let input = input.trim().to_lowercase();

    // Try relative dates first
    if let Some(date) = try_parse_relative(&input) {
        return Ok(date);
    }

    // Try weekday parsing
    if let Some(date) = try_parse_weekday(&input) {
        return Ok(date);
    }

    // Try offset parsing ("in X days/weeks")
    if let Some(date) = try_parse_offset(&input) {
        return Ok(date);
    }

    // Try ISO format
    if let Ok(date) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
        return Ok(date);
    }

    // Try various human-readable formats
    let formats = [
        "%b %d %Y", // Jan 25 2026
        "%B %d %Y", // January 25 2026
        "%b %d",    // Jan 25
        "%B %d",    // January 25
        "%m/%d/%Y", // 01/25/2026
        "%m/%d",    // 01/25
        "%d %b %Y", // 25 Jan 2026
        "%d %B %Y", // 25 January 2026
    ];

    for format in &formats {
        if let Ok(mut date) = NaiveDate::parse_from_str(&input, format) {
            // If year is not specified, add current year
            if !input.contains(|c: char| {
                c.is_numeric() && input.matches(|d: char| d.is_numeric()).count() > 4
            }) {
                let current_year = Local::now().year();
                date = date
                    .with_year(current_year)
                    .ok_or_else(|| CoreError::parse("Invalid date"))?;

                // If the date has passed this year, use next year
                if date < Local::now().date_naive() {
                    date = date
                        .with_year(current_year + 1)
                        .ok_or_else(|| CoreError::parse("Invalid date"))?;
                }
            }
            return Ok(date);
        }
    }

    Err(CoreError::parse(format!(
        "Could not parse date '{}'. Try formats like: 'tomorrow', 'Jan 25', '2026-01-25', 'next monday', 'in 3 days'",
        input
    )))
}

fn try_parse_relative(input: &str) -> Option<NaiveDate> {
    let today = Local::now().date_naive();

    match input {
        "today" => Some(today),
        "tomorrow" => today.checked_add_days(Days::new(1)),
        "yesterday" => today.checked_sub_days(Days::new(1)),
        _ => None,
    }
}

fn try_parse_weekday(input: &str) -> Option<NaiveDate> {
    let today = Local::now().date_naive();
    let input = input.trim();

    // Handle "next <weekday>"
    let (weekday_str, use_next_week) = if let Some(stripped) = input.strip_prefix("next ") {
        (stripped, true)
    } else {
        (input, false)
    };

    let target_weekday = match weekday_str {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    };

    let current_weekday = today.weekday();
    let days_until = if use_next_week {
        // Always go to next week
        let days = (target_weekday.num_days_from_monday() as i64
            - current_weekday.num_days_from_monday() as i64
            + 7)
            % 7;
        if days == 0 { 7 } else { days as u64 }
    } else {
        // Go to next occurrence (could be today if it matches)
        let days = (target_weekday.num_days_from_monday() as i64
            - current_weekday.num_days_from_monday() as i64
            + 7)
            % 7;
        if days == 0 { 7 } else { days as u64 }
    };

    today.checked_add_days(Days::new(days_until))
}

fn try_parse_offset(input: &str) -> Option<NaiveDate> {
    let today = Local::now().date_naive();
    let input = input.trim();

    // Match "in X day(s)" or "in X week(s)"
    if !input.starts_with("in ") {
        return None;
    }

    let rest = input.strip_prefix("in ")?.trim();
    let parts: Vec<&str> = rest.split_whitespace().collect();

    if parts.len() != 2 {
        return None;
    }

    let num: u64 = parts[0].parse().ok()?;
    let unit = parts[1].to_lowercase();

    match unit.as_str() {
        "day" | "days" => today.checked_add_days(Days::new(num)),
        "week" | "weeks" => today.checked_add_days(Days::new(num * 7)),
        _ => None,
    }
}

/// Format a NaiveDate for human-readable display
///
/// Returns strings like: "Today", "Tomorrow", "Mon Jan 27", "Overdue (3 days ago)"
pub fn format_date_human(date: NaiveDate, relative_to_today: bool) -> String {
    if !relative_to_today {
        return date.format("%Y-%m-%d").to_string();
    }

    let today = Local::now().date_naive();
    let diff = date.signed_duration_since(today).num_days();

    match diff {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        2..=6 => date.format("%a %b %d").to_string(),   // "Mon Jan 27"
        7..=365 => date.format("%b %d").to_string(),    // "Jan 27"
        _ if diff < 0 => format!("Overdue ({} days ago)", -diff),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_parse_today_tomorrow() {
        let today = Local::now().date_naive();
        assert_eq!(parse_date("today").unwrap(), today);
        assert_eq!(parse_date("tomorrow").unwrap(), today + Duration::days(1));
    }

    #[test]
    fn test_parse_iso_date() {
        let date = parse_date("2026-01-25").unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 1, 25).unwrap());
    }

    #[test]
    fn test_parse_weekday() {
        let result = parse_date("monday");
        assert!(result.is_ok());
        let date = result.unwrap();
        assert_eq!(date.weekday(), Weekday::Mon);
    }

    #[test]
    fn test_parse_next_weekday() {
        let result = parse_date("next friday");
        assert!(result.is_ok());
        let date = result.unwrap();
        assert_eq!(date.weekday(), Weekday::Fri);
    }

    #[test]
    fn test_parse_offset() {
        let today = Local::now().date_naive();
        assert_eq!(parse_date("in 3 days").unwrap(), today + Duration::days(3));
        assert_eq!(parse_date("in 1 week").unwrap(), today + Duration::days(7));
        assert_eq!(
            parse_date("in 2 weeks").unwrap(),
            today + Duration::days(14)
        );
    }

    #[test]
    fn test_format_date_human() {
        let today = Local::now().date_naive();
        assert_eq!(format_date_human(today, true), "Today");
        assert_eq!(
            format_date_human(today + Duration::days(1), true),
            "Tomorrow"
        );
        assert_eq!(
            format_date_human(today - Duration::days(1), true),
            "Yesterday"
        );
    }
}
