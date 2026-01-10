//! SEL Strategy Scheduler Implementation.
//!
//! This module provides scheduling functionality for SEL strategies,
//! supporting daily, weekly, monthly, and cron-based schedules.

#![allow(clippy::cast_lossless)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::use_self)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::significant_drop_in_scrutinee)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::option_if_let_else)]

use chrono::{DateTime, Datelike, NaiveTime, TimeZone, Timelike, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info, warn};

use zephyr_core::traits::ScheduleFrequency;
use zephyr_core::types::Timestamp;

/// Scheduler error types.
#[derive(Error, Debug)]
pub enum SchedulerError {
    /// Invalid schedule configuration.
    #[error("Invalid schedule: {0}")]
    InvalidSchedule(String),

    /// Cron expression parse error.
    #[error("Invalid cron expression: {0}")]
    InvalidCron(String),

    /// Scheduler already running.
    #[error("Scheduler already running")]
    AlreadyRunning,

    /// Scheduler not running.
    #[error("Scheduler not running")]
    NotRunning,

    /// Timezone error.
    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),
}

/// Scheduler configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Schedule frequency.
    pub frequency: ScheduleFrequency,
    /// Timezone for schedule (e.g., "UTC").
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Whether to run immediately on start.
    #[serde(default)]
    pub run_on_start: bool,
    /// Tolerance for schedule matching (in seconds).
    #[serde(default = "default_tolerance")]
    pub tolerance_seconds: u64,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_tolerance() -> u64 {
    60 // 1 minute tolerance
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            frequency: ScheduleFrequency::Daily { hour: 0, minute: 0 },
            timezone: default_timezone(),
            run_on_start: false,
            tolerance_seconds: default_tolerance(),
        }
    }
}

/// Parsed cron expression.
#[derive(Debug, Clone)]
pub struct CronExpression {
    /// Minutes (0-59).
    pub minutes: CronField,
    /// Hours (0-23).
    pub hours: CronField,
    /// Day of month (1-31).
    pub day_of_month: CronField,
    /// Month (1-12).
    pub month: CronField,
    /// Day of week (0-6, 0 = Sunday).
    pub day_of_week: CronField,
}

/// A field in a cron expression.
#[derive(Debug, Clone)]
pub enum CronField {
    /// Any value (*).
    Any,
    /// Specific value.
    Value(u8),
    /// Range of values (start-end).
    Range(u8, u8),
    /// List of values.
    List(Vec<u8>),
    /// Step values (*/step or start/step).
    Step(Box<CronField>, u8),
}

impl CronField {
    /// Checks if a value matches this field.
    pub fn matches(&self, value: u8) -> bool {
        match self {
            Self::Any => true,
            Self::Value(v) => *v == value,
            Self::Range(start, end) => value >= *start && value <= *end,
            Self::List(values) => values.contains(&value),
            Self::Step(base, step) => {
                if *step == 0 {
                    return false;
                }
                match base.as_ref() {
                    Self::Any => value % step == 0,
                    Self::Range(start, end) => {
                        value >= *start && value <= *end && (value - start) % step == 0
                    }
                    _ => base.matches(value),
                }
            }
        }
    }
}

impl CronExpression {
    /// Parses a cron expression string.
    ///
    /// Supports standard 5-field cron format:
    /// `minute hour day_of_month month day_of_week`
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError::InvalidCron` if the expression is invalid.
    pub fn parse(expr: &str) -> Result<Self, SchedulerError> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(SchedulerError::InvalidCron(format!(
                "Expected 5 fields, got {}",
                fields.len()
            )));
        }

        Ok(Self {
            minutes: Self::parse_field(fields[0], 0, 59)?,
            hours: Self::parse_field(fields[1], 0, 23)?,
            day_of_month: Self::parse_field(fields[2], 1, 31)?,
            month: Self::parse_field(fields[3], 1, 12)?,
            day_of_week: Self::parse_field(fields[4], 0, 6)?,
        })
    }

    /// Parses a single cron field.
    fn parse_field(field: &str, min: u8, max: u8) -> Result<CronField, SchedulerError> {
        // Handle step notation
        if field.contains('/') {
            let parts: Vec<&str> = field.split('/').collect();
            if parts.len() != 2 {
                return Err(SchedulerError::InvalidCron(format!(
                    "Invalid step notation: {}",
                    field
                )));
            }
            let base = Self::parse_field(parts[0], min, max)?;
            let step = parts[1]
                .parse::<u8>()
                .map_err(|_| SchedulerError::InvalidCron(format!("Invalid step: {}", parts[1])))?;
            return Ok(CronField::Step(Box::new(base), step));
        }

        // Handle any (*)
        if field == "*" {
            return Ok(CronField::Any);
        }

        // Handle list (1,2,3)
        if field.contains(',') {
            let values: Result<Vec<u8>, _> = field
                .split(',')
                .map(|v| {
                    v.parse::<u8>()
                        .map_err(|_| SchedulerError::InvalidCron(format!("Invalid value: {}", v)))
                })
                .collect();
            let values = values?;
            for v in &values {
                if *v < min || *v > max {
                    return Err(SchedulerError::InvalidCron(format!(
                        "Value {} out of range {}-{}",
                        v, min, max
                    )));
                }
            }
            return Ok(CronField::List(values));
        }

        // Handle range (1-5)
        if field.contains('-') {
            let parts: Vec<&str> = field.split('-').collect();
            if parts.len() != 2 {
                return Err(SchedulerError::InvalidCron(format!(
                    "Invalid range: {}",
                    field
                )));
            }
            let start = parts[0]
                .parse::<u8>()
                .map_err(|_| SchedulerError::InvalidCron(format!("Invalid start: {}", parts[0])))?;
            let end = parts[1]
                .parse::<u8>()
                .map_err(|_| SchedulerError::InvalidCron(format!("Invalid end: {}", parts[1])))?;
            if start < min || end > max || start > end {
                return Err(SchedulerError::InvalidCron(format!(
                    "Invalid range: {}-{}",
                    start, end
                )));
            }
            return Ok(CronField::Range(start, end));
        }

        // Handle single value
        let value = field
            .parse::<u8>()
            .map_err(|_| SchedulerError::InvalidCron(format!("Invalid value: {}", field)))?;
        if value < min || value > max {
            return Err(SchedulerError::InvalidCron(format!(
                "Value {} out of range {}-{}",
                value, min, max
            )));
        }
        Ok(CronField::Value(value))
    }

    /// Checks if the given time matches this cron expression.
    pub fn matches(&self, time: &DateTime<Utc>) -> bool {
        let minute = time.minute() as u8;
        let hour = time.hour() as u8;
        let day = time.day() as u8;
        let month = time.month() as u8;
        let weekday = time.weekday().num_days_from_sunday() as u8;

        self.minutes.matches(minute)
            && self.hours.matches(hour)
            && self.day_of_month.matches(day)
            && self.month.matches(month)
            && self.day_of_week.matches(weekday)
    }
}

/// Schedule matcher for determining when to trigger callbacks.
pub struct ScheduleMatcher {
    /// Schedule frequency.
    frequency: ScheduleFrequency,
    /// Parsed cron expression (if using cron).
    cron: Option<CronExpression>,
    /// Last trigger time.
    last_trigger: RwLock<Option<DateTime<Utc>>>,
    /// Tolerance in seconds.
    tolerance_seconds: u64,
}

impl ScheduleMatcher {
    /// Creates a new schedule matcher.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError` if the schedule is invalid.
    pub fn new(
        frequency: ScheduleFrequency,
        tolerance_seconds: u64,
    ) -> Result<Self, SchedulerError> {
        // Validate the frequency
        frequency
            .validate()
            .map_err(|e| SchedulerError::InvalidSchedule(e.to_string()))?;

        // Parse cron if needed
        let cron = if let ScheduleFrequency::Cron(expr) = &frequency {
            Some(CronExpression::parse(expr)?)
        } else {
            None
        };

        Ok(Self {
            frequency,
            cron,
            last_trigger: RwLock::new(None),
            tolerance_seconds,
        })
    }

    /// Checks if the schedule should trigger at the given time.
    pub fn should_trigger(&self, time: &DateTime<Utc>) -> bool {
        // Check if we already triggered recently
        if let Some(last) = *self.last_trigger.read() {
            let elapsed = time.signed_duration_since(last);
            if elapsed.num_seconds() < self.tolerance_seconds as i64 {
                return false;
            }
        }

        let matches = match &self.frequency {
            ScheduleFrequency::Daily { hour, minute } => {
                time.hour() as u8 == *hour && time.minute() as u8 == *minute
            }
            ScheduleFrequency::Weekly { day, hour, minute } => {
                time.weekday() == *day
                    && time.hour() as u8 == *hour
                    && time.minute() as u8 == *minute
            }
            ScheduleFrequency::Monthly { day, hour, minute } => {
                time.day() as u8 == *day
                    && time.hour() as u8 == *hour
                    && time.minute() as u8 == *minute
            }
            ScheduleFrequency::Cron(_) => {
                if let Some(cron) = &self.cron {
                    cron.matches(time)
                } else {
                    false
                }
            }
        };

        if matches {
            *self.last_trigger.write() = Some(*time);
        }

        matches
    }

    /// Resets the last trigger time.
    pub fn reset(&self) {
        *self.last_trigger.write() = None;
    }

    /// Gets the next scheduled time after the given time.
    ///
    /// Note: This is a simplified implementation that may not be perfectly accurate
    /// for all cron expressions.
    pub fn next_trigger(&self, after: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        match &self.frequency {
            ScheduleFrequency::Daily { hour, minute } => {
                let today = after.date_naive().and_time(NaiveTime::from_hms_opt(
                    *hour as u32,
                    *minute as u32,
                    0,
                )?);
                let today_utc = Utc.from_utc_datetime(&today);

                if today_utc > *after {
                    Some(today_utc)
                } else {
                    // Tomorrow
                    let tomorrow = today + chrono::Duration::days(1);
                    Some(Utc.from_utc_datetime(&tomorrow))
                }
            }
            ScheduleFrequency::Weekly { day, hour, minute } => {
                let target_weekday = *day;
                let current_weekday = after.weekday();
                let days_until = (target_weekday.num_days_from_monday() as i64
                    - current_weekday.num_days_from_monday() as i64
                    + 7)
                    % 7;

                let target_date = after.date_naive() + chrono::Duration::days(days_until);
                let target_time = NaiveTime::from_hms_opt(*hour as u32, *minute as u32, 0)?;
                let target = target_date.and_time(target_time);
                let target_utc = Utc.from_utc_datetime(&target);

                if target_utc > *after {
                    Some(target_utc)
                } else {
                    // Next week
                    let next_week = target + chrono::Duration::weeks(1);
                    Some(Utc.from_utc_datetime(&next_week))
                }
            }
            ScheduleFrequency::Monthly { day, hour, minute } => {
                let target_day = *day as u32;
                let target_time = NaiveTime::from_hms_opt(*hour as u32, *minute as u32, 0)?;

                // Try this month
                if let Some(target_date) = after.date_naive().with_day(target_day) {
                    let target = target_date.and_time(target_time);
                    let target_utc = Utc.from_utc_datetime(&target);
                    if target_utc > *after {
                        return Some(target_utc);
                    }
                }

                // Try next month
                let next_month = if after.month() == 12 {
                    after.with_year(after.year() + 1)?.with_month(1)?
                } else {
                    after.with_month(after.month() + 1)?
                };

                next_month
                    .date_naive()
                    .with_day(target_day)
                    .map(|d| Utc.from_utc_datetime(&d.and_time(target_time)))
            }
            ScheduleFrequency::Cron(_) => {
                // For cron, we'd need to iterate through time to find the next match
                // This is a simplified implementation
                None
            }
        }
    }
}

/// Scheduler event sent to strategy runners.
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    /// Trigger the scheduled callback.
    Trigger(Timestamp),
    /// Stop the scheduler.
    Stop,
}

/// SEL Strategy Scheduler.
///
/// Manages scheduled execution of SEL strategies.
pub struct SelScheduler {
    /// Scheduler configuration.
    config: SchedulerConfig,
    /// Schedule matcher.
    matcher: ScheduleMatcher,
    /// Whether the scheduler is running.
    running: RwLock<bool>,
    /// Event sender.
    event_tx: Option<mpsc::Sender<SchedulerEvent>>,
}

impl SelScheduler {
    /// Creates a new scheduler.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError` if the configuration is invalid.
    pub fn new(config: SchedulerConfig) -> Result<Self, SchedulerError> {
        let matcher = ScheduleMatcher::new(config.frequency.clone(), config.tolerance_seconds)?;

        Ok(Self {
            config,
            matcher,
            running: RwLock::new(false),
            event_tx: None,
        })
    }

    /// Creates a scheduler with an event channel.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError` if the configuration is invalid.
    pub fn with_channel(
        config: SchedulerConfig,
        event_tx: mpsc::Sender<SchedulerEvent>,
    ) -> Result<Self, SchedulerError> {
        let matcher = ScheduleMatcher::new(config.frequency.clone(), config.tolerance_seconds)?;

        Ok(Self {
            config,
            matcher,
            running: RwLock::new(false),
            event_tx: Some(event_tx),
        })
    }

    /// Checks if the scheduler should trigger now.
    pub fn should_trigger_now(&self) -> bool {
        let now = Utc::now();
        self.matcher.should_trigger(&now)
    }

    /// Gets the next scheduled time.
    pub fn next_trigger_time(&self) -> Option<DateTime<Utc>> {
        let now = Utc::now();
        self.matcher.next_trigger(&now)
    }

    /// Starts the scheduler loop.
    ///
    /// This spawns a background task that checks the schedule periodically.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError::AlreadyRunning` if already running.
    pub async fn start(&mut self) -> Result<(), SchedulerError> {
        if *self.running.read() {
            return Err(SchedulerError::AlreadyRunning);
        }

        *self.running.write() = true;

        // Send initial trigger if configured
        if self.config.run_on_start {
            if let Some(tx) = &self.event_tx {
                let now = Timestamp::now();
                if let Err(e) = tx.send(SchedulerEvent::Trigger(now)).await {
                    error!("Failed to send initial trigger: {}", e);
                }
            }
        }

        info!(
            frequency = %self.config.frequency,
            "Scheduler started"
        );

        Ok(())
    }

    /// Stops the scheduler.
    pub async fn stop(&mut self) {
        if !*self.running.read() {
            return;
        }

        *self.running.write() = false;

        if let Some(tx) = &self.event_tx {
            let _ = tx.send(SchedulerEvent::Stop).await;
        }

        info!("Scheduler stopped");
    }

    /// Returns whether the scheduler is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Manually triggers the schedule.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError::NotRunning` if not running.
    pub async fn trigger_now(&self) -> Result<(), SchedulerError> {
        if !*self.running.read() {
            return Err(SchedulerError::NotRunning);
        }

        if let Some(tx) = &self.event_tx {
            let now = Timestamp::now();
            tx.send(SchedulerEvent::Trigger(now))
                .await
                .map_err(|_| SchedulerError::NotRunning)?;
        }

        Ok(())
    }

    /// Runs the scheduler check loop.
    ///
    /// This should be called in a spawned task.
    pub async fn run_loop(&self) {
        let check_interval = Duration::from_secs(1); // Check every second
        let mut interval_timer = interval(check_interval);

        while *self.running.read() {
            interval_timer.tick().await;

            if self.should_trigger_now() {
                debug!("Schedule triggered");
                if let Some(tx) = &self.event_tx {
                    let now = Timestamp::now();
                    if let Err(e) = tx.send(SchedulerEvent::Trigger(now)).await {
                        warn!("Failed to send trigger event: {}", e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    #[test]
    fn test_cron_field_any() {
        let field = CronField::Any;
        assert!(field.matches(0));
        assert!(field.matches(30));
        assert!(field.matches(59));
    }

    #[test]
    fn test_cron_field_value() {
        let field = CronField::Value(30);
        assert!(!field.matches(0));
        assert!(field.matches(30));
        assert!(!field.matches(59));
    }

    #[test]
    fn test_cron_field_range() {
        let field = CronField::Range(10, 20);
        assert!(!field.matches(5));
        assert!(field.matches(10));
        assert!(field.matches(15));
        assert!(field.matches(20));
        assert!(!field.matches(25));
    }

    #[test]
    fn test_cron_field_list() {
        let field = CronField::List(vec![0, 15, 30, 45]);
        assert!(field.matches(0));
        assert!(!field.matches(10));
        assert!(field.matches(15));
        assert!(field.matches(30));
        assert!(field.matches(45));
        assert!(!field.matches(50));
    }

    #[test]
    fn test_cron_field_step() {
        let field = CronField::Step(Box::new(CronField::Any), 15);
        assert!(field.matches(0));
        assert!(!field.matches(10));
        assert!(field.matches(15));
        assert!(field.matches(30));
        assert!(field.matches(45));
        assert!(!field.matches(50));
    }

    #[test]
    fn test_cron_expression_parse() {
        // Every minute
        let cron = CronExpression::parse("* * * * *").unwrap();
        assert!(matches!(cron.minutes, CronField::Any));
        assert!(matches!(cron.hours, CronField::Any));

        // Every day at midnight
        let cron = CronExpression::parse("0 0 * * *").unwrap();
        assert!(matches!(cron.minutes, CronField::Value(0)));
        assert!(matches!(cron.hours, CronField::Value(0)));

        // Every 15 minutes
        let cron = CronExpression::parse("*/15 * * * *").unwrap();
        assert!(matches!(cron.minutes, CronField::Step(_, 15)));

        // Weekdays at 9am
        let cron = CronExpression::parse("0 9 * * 1-5").unwrap();
        assert!(matches!(cron.day_of_week, CronField::Range(1, 5)));
    }

    #[test]
    fn test_cron_expression_invalid() {
        // Too few fields
        assert!(CronExpression::parse("* * *").is_err());

        // Invalid value
        assert!(CronExpression::parse("60 * * * *").is_err());

        // Invalid range
        assert!(CronExpression::parse("* 25 * * *").is_err());
    }

    #[test]
    fn test_cron_expression_matches() {
        let cron = CronExpression::parse("30 9 * * *").unwrap();

        // 9:30 AM should match
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 9, 30, 0).unwrap();
        assert!(cron.matches(&time));

        // 9:31 AM should not match
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 9, 31, 0).unwrap();
        assert!(!cron.matches(&time));

        // 10:30 AM should not match
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        assert!(!cron.matches(&time));
    }

    #[test]
    fn test_schedule_matcher_daily() {
        let matcher = ScheduleMatcher::new(
            ScheduleFrequency::Daily {
                hour: 9,
                minute: 30,
            },
            60,
        )
        .unwrap();

        // 9:30 AM should trigger
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 9, 30, 0).unwrap();
        assert!(matcher.should_trigger(&time));

        // Should not trigger again within tolerance
        let time2 = Utc.with_ymd_and_hms(2024, 1, 15, 9, 30, 30).unwrap();
        assert!(!matcher.should_trigger(&time2));

        // Reset and try again
        matcher.reset();
        assert!(matcher.should_trigger(&time));
    }

    #[test]
    fn test_schedule_matcher_weekly() {
        let matcher = ScheduleMatcher::new(
            ScheduleFrequency::Weekly {
                day: Weekday::Mon,
                hour: 9,
                minute: 0,
            },
            60,
        )
        .unwrap();

        // Monday 9:00 AM should trigger
        let monday = Utc.with_ymd_and_hms(2024, 1, 15, 9, 0, 0).unwrap(); // Jan 15, 2024 is Monday
        assert!(matcher.should_trigger(&monday));

        // Tuesday 9:00 AM should not trigger
        let tuesday = Utc.with_ymd_and_hms(2024, 1, 16, 9, 0, 0).unwrap();
        matcher.reset();
        assert!(!matcher.should_trigger(&tuesday));
    }

    #[test]
    fn test_schedule_matcher_monthly() {
        let matcher = ScheduleMatcher::new(
            ScheduleFrequency::Monthly {
                day: 1,
                hour: 0,
                minute: 0,
            },
            60,
        )
        .unwrap();

        // 1st of month at midnight should trigger
        let first = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        assert!(matcher.should_trigger(&first));

        // 2nd of month should not trigger
        let second = Utc.with_ymd_and_hms(2024, 1, 2, 0, 0, 0).unwrap();
        matcher.reset();
        assert!(!matcher.should_trigger(&second));
    }

    #[test]
    fn test_schedule_matcher_cron() {
        let matcher =
            ScheduleMatcher::new(ScheduleFrequency::Cron("*/15 * * * *".to_string()), 60).unwrap();

        // :00 should trigger
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        assert!(matcher.should_trigger(&time));

        // :15 should trigger
        matcher.reset();
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 15, 0).unwrap();
        assert!(matcher.should_trigger(&time));

        // :10 should not trigger
        matcher.reset();
        let time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 10, 0).unwrap();
        assert!(!matcher.should_trigger(&time));
    }

    #[test]
    fn test_schedule_matcher_next_trigger_daily() {
        let matcher = ScheduleMatcher::new(
            ScheduleFrequency::Daily {
                hour: 9,
                minute: 30,
            },
            60,
        )
        .unwrap();

        // Before 9:30, next should be today
        let before = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
        let next = matcher.next_trigger(&before).unwrap();
        assert_eq!(next.hour(), 9);
        assert_eq!(next.minute(), 30);
        assert_eq!(next.day(), 15);

        // After 9:30, next should be tomorrow
        let after = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let next = matcher.next_trigger(&after).unwrap();
        assert_eq!(next.hour(), 9);
        assert_eq!(next.minute(), 30);
        assert_eq!(next.day(), 16);
    }

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.timezone, "UTC");
        assert!(!config.run_on_start);
        assert_eq!(config.tolerance_seconds, 60);
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let config = SchedulerConfig::default();
        let mut scheduler = SelScheduler::new(config).unwrap();

        assert!(!scheduler.is_running());

        scheduler.start().await.unwrap();
        assert!(scheduler.is_running());

        // Starting again should fail
        assert!(scheduler.start().await.is_err());

        scheduler.stop().await;
        assert!(!scheduler.is_running());
    }
}
