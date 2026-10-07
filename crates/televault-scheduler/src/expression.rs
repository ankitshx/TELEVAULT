//! Schedule expression parsing, validation, and deterministic next-run calculation.

use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use std::collections::BTreeSet;

use crate::error::SchedulerError;
use crate::types::{ScheduleType, TimezoneStrategy};

/// Maximum lookahead window in days for cron matching to prevent unbounded loops.
const MAX_CRON_LOOKAHEAD_DAYS: i64 = 366 * 5; // 5 years

/// Validates that a schedule expression is syntactically valid for the given schedule type.
pub fn validate_expression(schedule_type: ScheduleType, expr: &str) -> Result<(), SchedulerError> {
    match schedule_type {
        ScheduleType::Interval => {
            let secs = parse_interval_seconds(expr)?;
            if secs < 60 {
                return Err(SchedulerError::InvalidExpression(
                    "Minimum interval is 60 seconds (1 minute)".to_string(),
                ));
            }
            Ok(())
        }
        ScheduleType::Daily => {
            parse_daily_time(expr)?;
            Ok(())
        }
        ScheduleType::Weekly => {
            parse_weekly_time(expr)?;
            Ok(())
        }
        ScheduleType::Cron => {
            CronMatcher::parse(expr)?;
            Ok(())
        }
    }
}

/// Parses an interval expression string (e.g. "15m", "1h", "6h", "24h", "1d", or seconds "3600") into seconds.
pub fn parse_interval_seconds(expr: &str) -> Result<i64, SchedulerError> {
    let s = expr.trim();
    if s.is_empty() {
        return Err(SchedulerError::InvalidExpression(
            "Interval expression cannot be empty".to_string(),
        ));
    }

    if let Ok(secs) = s.parse::<i64>() {
        if secs <= 0 {
            return Err(SchedulerError::InvalidExpression(
                "Interval in seconds must be positive".to_string(),
            ));
        }
        return Ok(secs);
    }

    let (val_str, unit) = s.split_at(s.len() - 1);
    let val: i64 = val_str.trim().parse().map_err(|_| {
        SchedulerError::InvalidExpression(format!(
            "Invalid numeric value in interval expression '{s}'"
        ))
    })?;

    if val <= 0 {
        return Err(SchedulerError::InvalidExpression(
            "Interval value must be positive".to_string(),
        ));
    }

    match unit.to_lowercase().as_str() {
        "s" => Ok(val),
        "m" => Ok(val * 60),
        "h" => Ok(val * 3600),
        "d" => Ok(val * 86400),
        other => Err(SchedulerError::InvalidExpression(format!(
            "Unknown interval unit '{other}' in '{s}'. Supported: s, m, h, d"
        ))),
    }
}

/// Parses a daily time expression string in "HH:MM" 24-hour format.
pub fn parse_daily_time(expr: &str) -> Result<(u32, u32), SchedulerError> {
    let parts: Vec<&str> = expr.trim().split(':').collect();
    if parts.len() != 2 {
        return Err(SchedulerError::InvalidExpression(format!(
            "Daily time must be in 'HH:MM' 24-hour format (e.g. '02:00'), found: '{expr}'"
        )));
    }

    let hour: u32 = parts[0].trim().parse().map_err(|_| {
        SchedulerError::InvalidExpression(format!(
            "Invalid hour in daily expression: '{}'",
            parts[0]
        ))
    })?;
    let minute: u32 = parts[1].trim().parse().map_err(|_| {
        SchedulerError::InvalidExpression(format!(
            "Invalid minute in daily expression: '{}'",
            parts[1]
        ))
    })?;

    if hour > 23 || minute > 59 {
        return Err(SchedulerError::InvalidExpression(format!(
            "Daily time out of range: hour must be 0-23, minute 0-59 (got {hour:02}:{minute:02})"
        )));
    }

    Ok((hour, minute))
}

/// Parses a weekly time expression string in "Weekday@HH:MM" or "Weekday HH:MM" format.
pub fn parse_weekly_time(expr: &str) -> Result<(Weekday, u32, u32), SchedulerError> {
    let s = expr.trim();
    let delimiter = if s.contains('@') {
        '@'
    } else if s.contains(' ') {
        ' '
    } else {
        return Err(SchedulerError::InvalidExpression(format!(
            "Weekly time must be in 'Weekday@HH:MM' format (e.g. 'Sun@03:00'), found: '{s}'"
        )));
    };

    let parts: Vec<&str> = s.split(delimiter).collect();
    if parts.len() != 2 {
        return Err(SchedulerError::InvalidExpression(format!(
            "Weekly time must be in 'Weekday@HH:MM' format (e.g. 'Sun@03:00'), found: '{s}'"
        )));
    }

    let weekday = parse_weekday(parts[0].trim())?;
    let (hour, minute) = parse_daily_time(parts[1].trim())?;

    Ok((weekday, hour, minute))
}

/// Parses standard weekday names or three-letter abbreviations (case-insensitive).
pub fn parse_weekday(s: &str) -> Result<Weekday, SchedulerError> {
    match s.trim().to_lowercase().as_str() {
        "mon" | "monday" => Ok(Weekday::Mon),
        "tue" | "tuesday" => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thursday" => Ok(Weekday::Thu),
        "fri" | "friday" => Ok(Weekday::Fri),
        "sat" | "saturday" => Ok(Weekday::Sat),
        "sun" | "sunday" => Ok(Weekday::Sun),
        other => Err(SchedulerError::InvalidExpression(format!(
            "Unknown weekday '{other}'. Expected Monday..Sunday or Mon..Sun"
        ))),
    }
}

/// Deterministic next-run calculation from reference time `from_utc`.
pub fn calculate_next_run(
    schedule_type: ScheduleType,
    expression: &str,
    timezone: TimezoneStrategy,
    from_utc: DateTime<Utc>,
) -> Result<DateTime<Utc>, SchedulerError> {
    match schedule_type {
        ScheduleType::Interval => {
            let secs = parse_interval_seconds(expression)?;
            Ok(from_utc + Duration::seconds(secs))
        }
        ScheduleType::Daily => {
            let (target_h, target_m) = parse_daily_time(expression)?;
            match timezone {
                TimezoneStrategy::Utc => {
                    let from_date = from_utc.date_naive();
                    let target_time = NaiveTime::from_hms_opt(target_h, target_m, 0).unwrap();
                    let target_dt = from_date.and_time(target_time).and_utc();

                    if target_dt > from_utc {
                        Ok(target_dt)
                    } else {
                        Ok((from_date + Duration::days(1))
                            .and_time(target_time)
                            .and_utc())
                    }
                }
                TimezoneStrategy::Local => {
                    let local_from: DateTime<Local> =
                        Local.from_utc_datetime(&from_utc.naive_utc());
                    let from_date = local_from.date_naive();
                    let target_time = NaiveTime::from_hms_opt(target_h, target_m, 0).unwrap();

                    let candidate = from_date.and_time(target_time);
                    let candidate_local = match Local.from_local_datetime(&candidate) {
                        chrono::LocalResult::Single(dt) => dt,
                        chrono::LocalResult::Ambiguous(earliest, _) => earliest,
                        chrono::LocalResult::None => {
                            // In DST spring-forward gap, advance 1 hour safely
                            Local
                                .from_local_datetime(&(candidate + Duration::hours(1)))
                                .single()
                                .ok_or_else(|| {
                                    SchedulerError::InvalidSchedule(
                                        "Ambiguous DST transition".into(),
                                    )
                                })?
                        }
                    };

                    if candidate_local > local_from {
                        Ok(candidate_local.with_timezone(&Utc))
                    } else {
                        let next_date = from_date + Duration::days(1);
                        let next_candidate = next_date.and_time(target_time);
                        let next_local = match Local.from_local_datetime(&next_candidate) {
                            chrono::LocalResult::Single(dt) => dt,
                            chrono::LocalResult::Ambiguous(earliest, _) => earliest,
                            chrono::LocalResult::None => Local
                                .from_local_datetime(&(next_candidate + Duration::hours(1)))
                                .single()
                                .ok_or_else(|| {
                                    SchedulerError::InvalidSchedule(
                                        "Ambiguous DST transition".into(),
                                    )
                                })?,
                        };
                        Ok(next_local.with_timezone(&Utc))
                    }
                }
            }
        }
        ScheduleType::Weekly => {
            let (target_weekday, target_h, target_m) = parse_weekly_time(expression)?;
            match timezone {
                TimezoneStrategy::Utc => {
                    let mut current_date = from_utc.date_naive();
                    let target_time = NaiveTime::from_hms_opt(target_h, target_m, 0).unwrap();

                    // Advance day-by-day until target weekday
                    for _ in 0..7 {
                        let candidate = current_date.and_time(target_time).and_utc();
                        if current_date.weekday() == target_weekday && candidate > from_utc {
                            return Ok(candidate);
                        }
                        current_date += Duration::days(1);
                    }
                    // Next week occurrence
                    Ok(current_date.and_time(target_time).and_utc())
                }
                TimezoneStrategy::Local => {
                    let local_from: DateTime<Local> =
                        Local.from_utc_datetime(&from_utc.naive_utc());
                    let mut current_date = local_from.date_naive();
                    let target_time = NaiveTime::from_hms_opt(target_h, target_m, 0).unwrap();

                    for _ in 0..7 {
                        if current_date.weekday() == target_weekday {
                            let candidate = current_date.and_time(target_time);
                            if let chrono::LocalResult::Single(dt) =
                                Local.from_local_datetime(&candidate)
                            {
                                if dt > local_from {
                                    return Ok(dt.with_timezone(&Utc));
                                }
                            }
                        }
                        current_date += Duration::days(1);
                    }

                    // Fallback to exactly 7 days later
                    let next_date = current_date;
                    let candidate = next_date.and_time(target_time);
                    let dt = Local
                        .from_local_datetime(&candidate)
                        .single()
                        .ok_or_else(|| {
                            SchedulerError::InvalidSchedule(
                                "Invalid local time for weekly schedule".into(),
                            )
                        })?;
                    Ok(dt.with_timezone(&Utc))
                }
            }
        }
        ScheduleType::Cron => {
            let matcher = CronMatcher::parse(expression)?;
            match timezone {
                TimezoneStrategy::Utc => matcher.find_next_utc(from_utc),
                TimezoneStrategy::Local => matcher.find_next_local(from_utc),
            }
        }
    }
}

/// Standard 5-field cron matcher for (minute, hour, dom, month, dow).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronMatcher {
    minutes: BTreeSet<u32>,       // 0..59
    hours: BTreeSet<u32>,         // 0..23
    days_of_month: BTreeSet<u32>, // 1..31
    months: BTreeSet<u32>,        // 1..12
    days_of_week: BTreeSet<u32>,  // 0..6 (0 = Sunday, 1 = Monday, ..., 6 = Saturday)
}

impl CronMatcher {
    /// Parses a standard 5-part cron expression.
    pub fn parse(expr: &str) -> Result<Self, SchedulerError> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(SchedulerError::InvalidExpression(format!(
                "Cron expression must contain exactly 5 space-separated fields (minute, hour, dom, month, dow), found: '{expr}'"
            )));
        }

        let minutes = parse_cron_field(parts[0], 0, 59, "minute")?;
        let hours = parse_cron_field(parts[1], 0, 23, "hour")?;
        let days_of_month = parse_cron_field(parts[2], 1, 31, "day-of-month")?;
        let months = parse_cron_field(parts[3], 1, 12, "month")?;
        let raw_dow = parse_cron_field(parts[4], 0, 7, "day-of-week")?;

        // Normalize 7 (Sunday) to 0
        let mut days_of_week = BTreeSet::new();
        for d in raw_dow {
            if d == 7 {
                days_of_week.insert(0);
            } else {
                days_of_week.insert(d);
            }
        }

        Ok(Self {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
        })
    }

    /// Finds the next matching UTC timestamp starting strictly after `from_utc`.
    pub fn find_next_utc(&self, from_utc: DateTime<Utc>) -> Result<DateTime<Utc>, SchedulerError> {
        // Truncate seconds and advance to the next whole minute
        let start = from_utc
            .with_second(0)
            .and_then(|dt| dt.with_nanosecond(0))
            .unwrap_or(from_utc)
            + Duration::minutes(1);

        let mut current = start;
        let limit = start + Duration::days(MAX_CRON_LOOKAHEAD_DAYS);

        while current < limit {
            let m = current.minute();
            let h = current.hour();
            let dom = current.day();
            let month = current.month();
            let dow = current.weekday().num_days_from_sunday();

            if self.months.contains(&month)
                && self.days_of_month.contains(&dom)
                && self.days_of_week.contains(&dow)
                && self.hours.contains(&h)
                && self.minutes.contains(&m)
            {
                return Ok(current);
            }

            // Optimization: if month or day doesn't match, advance by day/hour rather than single minute
            if !self.months.contains(&month)
                || !self.days_of_month.contains(&dom)
                || !self.days_of_week.contains(&dow)
            {
                current = (current.date_naive() + Duration::days(1))
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc();
            } else if !self.hours.contains(&h) {
                current = (current + Duration::hours(1)).with_minute(0).unwrap();
            } else {
                current += Duration::minutes(1);
            }
        }

        Err(SchedulerError::InvalidExpression(
            "No matching future time found for cron expression within horizon".to_string(),
        ))
    }

    /// Finds the next matching Local timestamp converted to UTC.
    pub fn find_next_local(
        &self,
        from_utc: DateTime<Utc>,
    ) -> Result<DateTime<Utc>, SchedulerError> {
        let local_from: DateTime<Local> = Local.from_utc_datetime(&from_utc.naive_utc());
        let start = local_from
            .with_second(0)
            .and_then(|dt| dt.with_nanosecond(0))
            .unwrap_or(local_from)
            + Duration::minutes(1);

        let mut current = start;
        let limit = start + Duration::days(MAX_CRON_LOOKAHEAD_DAYS);

        while current < limit {
            let m = current.minute();
            let h = current.hour();
            let dom = current.day();
            let month = current.month();
            let dow = current.weekday().num_days_from_sunday();

            if self.months.contains(&month)
                && self.days_of_month.contains(&dom)
                && self.days_of_week.contains(&dow)
                && self.hours.contains(&h)
                && self.minutes.contains(&m)
            {
                return Ok(current.with_timezone(&Utc));
            }

            if !self.months.contains(&month)
                || !self.days_of_month.contains(&dom)
                || !self.days_of_week.contains(&dow)
            {
                let next_day = current.date_naive() + Duration::days(1);
                let naive = next_day.and_hms_opt(0, 0, 0).unwrap();
                if let chrono::LocalResult::Single(dt) = Local.from_local_datetime(&naive) {
                    current = dt;
                } else {
                    current += Duration::hours(1);
                }
            } else if !self.hours.contains(&h) {
                current = (current + Duration::hours(1)).with_minute(0).unwrap();
            } else {
                current += Duration::minutes(1);
            }
        }

        Err(SchedulerError::InvalidExpression(
            "No matching future local time found for cron expression within horizon".to_string(),
        ))
    }
}

/// Parses an individual cron field supporting `*`, `*/step`, numbers, ranges, and lists.
fn parse_cron_field(
    field: &str,
    min: u32,
    max: u32,
    field_name: &str,
) -> Result<BTreeSet<u32>, SchedulerError> {
    let mut values = BTreeSet::new();

    for item in field.split(',') {
        let item = item.trim();
        if item == "*" {
            for v in min..=max {
                values.insert(v);
            }
        } else if let Some(step_str) = item.strip_prefix("*/") {
            let step: u32 = step_str.parse().map_err(|_| {
                SchedulerError::InvalidExpression(format!(
                    "Invalid step '{step_str}' in cron {field_name} field"
                ))
            })?;
            if step == 0 {
                return Err(SchedulerError::InvalidExpression(format!(
                    "Step cannot be 0 in cron {field_name} field"
                )));
            }
            let mut v = min;
            while v <= max {
                values.insert(v);
                v += step;
            }
        } else if item.contains('-') {
            let parts: Vec<&str> = item.split('-').collect();
            if parts.len() != 2 {
                return Err(SchedulerError::InvalidExpression(format!(
                    "Invalid range '{item}' in cron {field_name} field"
                )));
            }
            let start: u32 = parts[0].parse().map_err(|_| {
                SchedulerError::InvalidExpression(format!(
                    "Invalid range start '{}' in cron {field_name} field",
                    parts[0]
                ))
            })?;
            let end: u32 = parts[1].parse().map_err(|_| {
                SchedulerError::InvalidExpression(format!(
                    "Invalid range end '{}' in cron {field_name} field",
                    parts[1]
                ))
            })?;
            if start > end || start < min || end > max {
                return Err(SchedulerError::InvalidExpression(format!(
                    "Range '{start}-{end}' out of bounds ({min}..{max}) in cron {field_name} field"
                )));
            }
            for v in start..=end {
                values.insert(v);
            }
        } else {
            let val: u32 = item.parse().map_err(|_| {
                SchedulerError::InvalidExpression(format!(
                    "Invalid value '{item}' in cron {field_name} field"
                ))
            })?;
            if val < min || val > max {
                return Err(SchedulerError::InvalidExpression(format!(
                    "Value '{val}' out of bounds ({min}..{max}) in cron {field_name} field"
                )));
            }
            values.insert(val);
        }
    }

    if values.is_empty() {
        return Err(SchedulerError::InvalidExpression(format!(
            "Empty values for cron {field_name} field"
        )));
    }

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interval_seconds() {
        assert_eq!(parse_interval_seconds("15m").unwrap(), 900);
        assert_eq!(parse_interval_seconds("1h").unwrap(), 3600);
        assert_eq!(parse_interval_seconds("6h").unwrap(), 21600);
        assert_eq!(parse_interval_seconds("24h").unwrap(), 86400);
        assert_eq!(parse_interval_seconds("1d").unwrap(), 86400);
        assert_eq!(parse_interval_seconds("300").unwrap(), 300);
        assert!(parse_interval_seconds("0m").is_err());
        assert!(parse_interval_seconds("-15m").is_err());
        assert!(parse_interval_seconds("invalid").is_err());
    }

    #[test]
    fn test_parse_daily_time() {
        assert_eq!(parse_daily_time("02:00").unwrap(), (2, 0));
        assert_eq!(parse_daily_time("23:59").unwrap(), (23, 59));
        assert_eq!(parse_daily_time("00:00").unwrap(), (0, 0));
        assert!(parse_daily_time("24:00").is_err());
        assert!(parse_daily_time("12:60").is_err());
        assert!(parse_daily_time("invalid").is_err());
    }

    #[test]
    fn test_parse_weekly_time() {
        assert_eq!(
            parse_weekly_time("Sun@03:00").unwrap(),
            (Weekday::Sun, 3, 0)
        );
        assert_eq!(
            parse_weekly_time("Monday 14:30").unwrap(),
            (Weekday::Mon, 14, 30)
        );
        assert!(parse_weekly_time("Funday@03:00").is_err());
        assert!(parse_weekly_time("Sun@25:00").is_err());
    }

    #[test]
    fn test_cron_matcher_parsing_and_matching() {
        let matcher = CronMatcher::parse("*/15 * * * *").unwrap();
        assert_eq!(matcher.minutes.len(), 4); // 0, 15, 30, 45
        assert_eq!(matcher.hours.len(), 24);

        let matcher2 = CronMatcher::parse("0 2 * * *").unwrap();
        assert_eq!(matcher2.minutes, BTreeSet::from([0]));
        assert_eq!(matcher2.hours, BTreeSet::from([2]));

        assert!(CronMatcher::parse("* * *").is_err());
        assert!(CronMatcher::parse("60 * * * *").is_err());
        assert!(CronMatcher::parse("* 25 * * *").is_err());
    }

    #[test]
    fn test_calculate_next_run_deterministic() {
        let base = Utc.with_ymd_and_hms(2026, 10, 7, 10, 0, 0).unwrap();

        // Interval 1h -> 11:00
        let next_interval =
            calculate_next_run(ScheduleType::Interval, "1h", TimezoneStrategy::Utc, base).unwrap();
        assert_eq!(
            next_interval,
            Utc.with_ymd_and_hms(2026, 10, 7, 11, 0, 0).unwrap()
        );

        // Daily 14:00 (in future today) -> 14:00 today
        let next_daily =
            calculate_next_run(ScheduleType::Daily, "14:00", TimezoneStrategy::Utc, base).unwrap();
        assert_eq!(
            next_daily,
            Utc.with_ymd_and_hms(2026, 10, 7, 14, 0, 0).unwrap()
        );

        // Daily 02:00 (in past today) -> 02:00 tomorrow
        let next_daily_past =
            calculate_next_run(ScheduleType::Daily, "02:00", TimezoneStrategy::Utc, base).unwrap();
        assert_eq!(
            next_daily_past,
            Utc.with_ymd_and_hms(2026, 10, 8, 2, 0, 0).unwrap()
        );

        // Cron 0 2 * * * -> 02:00 tomorrow
        let next_cron =
            calculate_next_run(ScheduleType::Cron, "0 2 * * *", TimezoneStrategy::Utc, base)
                .unwrap();
        assert_eq!(
            next_cron,
            Utc.with_ymd_and_hms(2026, 10, 8, 2, 0, 0).unwrap()
        );
    }
}
