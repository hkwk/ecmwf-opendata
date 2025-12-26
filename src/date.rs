use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};

use crate::error::{Error, Result};

/// Accepts 0/6/12/18, 0000/0600/1200/1800, and string forms like "12" or "1200".
pub fn canonical_time_to_hour(time: &str) -> Result<u32> {
    let t = time.trim();
    let n: i64 = t
        .parse()
        .map_err(|_| Error::InvalidRequest(format!("invalid time value: {time}")))?;

    let hour = match n {
        0 | 6 | 12 | 18 => n as u32,
        600 => 6,
        1200 => 12,
        1800 => 18,
        _ => {
            return Err(Error::InvalidRequest(format!(
                "time must be one of 0,6,12,18 (or 0000/0600/1200/1800), got {n}"
            )));
        }
    };

    Ok(hour)
}

pub fn expand_time_value(v: &str) -> Result<Vec<String>> {
    // Upstream expands 0/to/18 into 0,6,12,18.
    // We'll implement numeric range expansion then filter to multiples of 6.
    let expanded = crate::request::expand_numeric_syntax(v)?;
    if v.contains("/to/") {
        let mut out = Vec::new();
        for s in &expanded {
            let n: i64 = s
                .parse()
                .map_err(|_| Error::InvalidRequest(format!("invalid time element: {s}")))?;
            if [0, 6, 12, 18].contains(&n) {
                out.push(n.to_string());
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    Ok(expanded)
}

pub fn yyyymmdd(date: &NaiveDate) -> String {
    format!("{:04}{:02}{:02}", date.year(), date.month(), date.day())
}

/// Parse date inputs similar to upstream:
/// - "YYYYMMDD" or "YYYY-MM-DD" or "YYYY-MM-DD HH:MM:SS"
/// - integer <= 0 means today + delta days
pub fn parse_date_like(s: &str, now: DateTime<Utc>) -> Result<(NaiveDate, Option<u32>)> {
    let trimmed = s.trim();
    if let Ok(n) = trimmed.parse::<i64>() {
        if n <= 0 {
            let d = now.date_naive() + Duration::days(n);
            return Ok((d, None));
        }
        // YYYYMMDD
        if trimmed.len() == 8 {
            let year: i32 = trimmed[0..4].parse().map_err(|_| {
                Error::InvalidRequest(format!("invalid YYYYMMDD date: {trimmed}"))
            })?;
            let month: u32 = trimmed[4..6].parse().map_err(|_| {
                Error::InvalidRequest(format!("invalid YYYYMMDD date: {trimmed}"))
            })?;
            let day: u32 = trimmed[6..8].parse().map_err(|_| {
                Error::InvalidRequest(format!("invalid YYYYMMDD date: {trimmed}"))
            })?;
            let d = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
                Error::InvalidRequest(format!("invalid date components: {trimmed}"))
            })?;
            return Ok((d, None));
        }
    }

    // YYYY-MM-DD or full timestamp
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok((d, None));
    }

    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Ok((dt.date(), Some(dt.hour())));
    }

    Err(Error::InvalidRequest(format!(
        "unsupported date format: {trimmed}"
    )))
}

pub fn expand_date_value(v: &str, now: DateTime<Utc>) -> Result<Vec<String>> {
    // Support range syntax: YYYYMMDD/to/YYYYMMDD[/by/N]
    if v.contains("/to/") {
        let tokens: Vec<&str> = v.split('/').collect();
        if tokens.len() == 3 && tokens[1].eq_ignore_ascii_case("to") {
            let (start, _) = parse_date_like(tokens[0], now)?;
            let (end, _) = parse_date_like(tokens[2], now)?;
            let mut out = Vec::new();
            let mut cur = start;
            while cur <= end {
                out.push(yyyymmdd(&cur));
                cur += Duration::days(1);
            }
            return Ok(out);
        }
        if tokens.len() == 5 && tokens[1].eq_ignore_ascii_case("to") && tokens[3].eq_ignore_ascii_case("by") {
            let (start, _) = parse_date_like(tokens[0], now)?;
            let (end, _) = parse_date_like(tokens[2], now)?;
            let by: i64 = tokens[4].parse().map_err(|_| {
                Error::InvalidRequest(format!("invalid date range step: {v}"))
            })?;
            if by <= 0 {
                return Err(Error::InvalidRequest(format!(
                    "date range step must be >0, got {by}"
                )));
            }
            let mut out = Vec::new();
            let mut cur = start;
            while cur <= end {
                out.push(yyyymmdd(&cur));
                cur += Duration::days(by);
            }
            return Ok(out);
        }
    }

    // Single date-like value
    let (d, _) = parse_date_like(v, now)?;
    Ok(vec![yyyymmdd(&d)])
}

pub fn full_datetime_from_date_time(
    date_yyyymmdd: &str,
    time_hour: u32,
) -> Result<DateTime<Utc>> {
    if date_yyyymmdd.len() != 8 {
        return Err(Error::InvalidRequest(format!(
            "date must be YYYYMMDD, got {date_yyyymmdd}"
        )));
    }
    let year: i32 = date_yyyymmdd[0..4].parse().map_err(|_| {
        Error::InvalidRequest(format!("invalid date: {date_yyyymmdd}"))
    })?;
    let month: u32 = date_yyyymmdd[4..6].parse().map_err(|_| {
        Error::InvalidRequest(format!("invalid date: {date_yyyymmdd}"))
    })?;
    let day: u32 = date_yyyymmdd[6..8].parse().map_err(|_| {
        Error::InvalidRequest(format!("invalid date: {date_yyyymmdd}"))
    })?;

    let d = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| Error::InvalidRequest(format!("invalid date: {date_yyyymmdd}")))?;

    Ok(Utc
        .with_ymd_and_hms(d.year(), d.month(), d.day(), time_hour, 0, 0)
        .single()
        .ok_or_else(|| Error::InvalidRequest("invalid datetime".into()))?)
}

/// For probability steps like "0-24" return the end portion.
pub fn end_step(step: &str) -> Option<i64> {
    if let Some((_, rhs)) = step.split_once('-') {
        rhs.trim().parse::<i64>().ok()
    } else {
        step.trim().parse::<i64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn canonical_time_parses_hours() {
        assert_eq!(canonical_time_to_hour("0").unwrap(), 0);
        assert_eq!(canonical_time_to_hour("6").unwrap(), 6);
        assert_eq!(canonical_time_to_hour("600").unwrap(), 6);
        assert_eq!(canonical_time_to_hour("1200").unwrap(), 12);
        assert_eq!(canonical_time_to_hour("18").unwrap(), 18);
    }

    #[test]
    fn expands_time_0_to_18() {
        assert_eq!(
            expand_time_value("0/to/18").unwrap(),
            vec!["0", "6", "12", "18"]
        );
    }

    #[test]
    fn expands_date_ranges() {
        let now = Utc.with_ymd_and_hms(2022, 1, 31, 12, 0, 0).unwrap();
        assert_eq!(
            expand_date_value("20000101/to/20000103", now).unwrap(),
            vec!["20000101", "20000102", "20000103"]
        );
        assert_eq!(
            expand_date_value("20000101/to/20000108/by/7", now).unwrap(),
            vec!["20000101", "20000108"]
        );
    }
}
