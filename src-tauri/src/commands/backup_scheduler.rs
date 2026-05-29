use crate::commands::backup::do_backup_with_type;
use crate::commands::backup_settings::{apply_retention, load_run_state, save_run_state};
use crate::db::DbState;
use crate::models::{BackupRunState, BackupSettings};
use chrono::{Datelike, TimeZone};
use std::path::Path;
use tokio::sync::{watch, Mutex};

pub struct BackupSchedulerHandle {
    pub settings_tx: watch::Sender<Option<BackupSettings>>,
    pub stop_tx: watch::Sender<bool>,
}

pub struct BackupSchedulerState {
    pub handle: Mutex<Option<BackupSchedulerHandle>>,
}

impl BackupSchedulerState {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
        }
    }

    pub async fn restart(&self, db: &DbState, settings: BackupSettings) {
        let mut handle_guard = self.handle.lock().await;

        // Stop existing scheduler if any
        if let Some(old) = handle_guard.take() {
            let _ = old.stop_tx.send(true);
        }

        if !settings.enabled || settings.target_dir.is_none() {
            return;
        }

        let (settings_tx, settings_rx) = watch::channel(Some(settings));
        let (stop_tx, stop_rx) = watch::channel(false);

        let db_clone = db.clone();
        tokio::spawn(run_scheduler(db_clone, settings_rx, stop_rx));

        *handle_guard = Some(BackupSchedulerHandle {
            settings_tx,
            stop_tx,
        });
    }

    pub async fn update_settings(&self, settings: BackupSettings) {
        let handle_guard = self.handle.lock().await;
        if let Some(handle) = handle_guard.as_ref() {
            let _ = handle.settings_tx.send(Some(settings));
        }
    }
}

fn parse_time(time_of_day: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = time_of_day.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

fn already_ran_this_period(run_state: &BackupRunState, now: &chrono::DateTime<chrono::Local>, settings: &BackupSettings) -> bool {
    // Use last_auto_run_at for scheduling decisions, not last_run_at
    // This prevents manual backups from interfering with the auto schedule
    let last_auto_at = match &run_state.last_auto_run_at {
        Some(t) => t,
        None => return false,
    };
    let last_dt = match chrono::NaiveDateTime::parse_from_str(last_auto_at, "%Y-%m-%d %H:%M:%S") {
        Ok(dt) => chrono::Local.from_local_datetime(&dt).single().unwrap(),
        Err(_) => return false,
    };

    match settings.frequency.as_str() {
        "interval_minutes" | "interval_hours" => {
            let interval = chrono::Duration::minutes(settings.interval_minutes.unwrap_or(60) as i64);
            *now - last_dt < interval
        }
        "weekly" => {
            let target_weekday = settings.day_of_week.unwrap_or(1);
            let now_weekday = now.date_naive().weekday().num_days_from_monday() + 1;
            if now_weekday >= target_weekday {
                last_dt.date_naive() >= now.date_naive() - chrono::Duration::days((now_weekday - target_weekday) as i64)
            } else {
                last_dt.date_naive() >= now.date_naive() - chrono::Duration::days((7 - target_weekday + now_weekday) as i64)
            }
        }
        "monthly" => {
            let target_day = settings.day_of_month.unwrap_or(1);
            let today = now.date_naive();
            let this_month_target = if today.day() >= target_day {
                chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), target_day.min(last_day_of_month(today.year(), today.month())))
            } else {
                let (y, m) = if today.month() == 1 {
                    (today.year() - 1, 12)
                } else {
                    (today.year(), today.month() - 1)
                };
                chrono::NaiveDate::from_ymd_opt(y, m, target_day.min(last_day_of_month(y, m)))
            };
            match this_month_target {
                Some(target_date) => last_dt.date_naive() >= target_date,
                None => false,
            }
        }
        _ => {
            // daily
            last_dt.date_naive() == now.date_naive()
        }
    }
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    chrono::NaiveDate::from_ymd_opt(y, m, 1)
        .map(|d| (d - chrono::Duration::days(1)).day())
        .unwrap_or(28)
}

async fn run_scheduler(
    db: DbState,
    mut settings_rx: watch::Receiver<Option<BackupSettings>>,
    mut stop_rx: watch::Receiver<bool>,
) {
    // Wait a bit after app start before first check
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    loop {
        if *stop_rx.borrow() {
            break;
        }

        let current_settings = settings_rx.borrow().clone();
        let settings = match current_settings {
            Some(s) if s.enabled && s.target_dir.is_some() => s,
            _ => {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            }
        };

        let now = chrono::Local::now();

        let is_interval = matches!(settings.frequency.as_str(), "interval_minutes" | "interval_hours");

        let should_run = if is_interval {
            // Interval-based: check if enough time has passed since last run
            let run_state = load_run_state(&db.app_data_dir);
            !already_ran_this_period(&run_state, &now, &settings)
        } else {
            // Time-of-day based: check if we've passed the target time today
            let (hour, minute) = match parse_time(&settings.time_of_day) {
                Some(t) => t,
                None => {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
            };

            let today_target = now
                .date_naive()
                .and_hms_opt(hour, minute, 0)
                .unwrap();
            let today_target = chrono::Local
                .from_local_datetime(&today_target)
                .single()
                .unwrap();

            if now < today_target {
                false
            } else {
                // Check if it's the right day for weekly/monthly
                let is_right_day = match settings.frequency.as_str() {
                    "weekly" => {
                        let target_weekday = settings.day_of_week.unwrap_or(1);
                        now.date_naive().weekday().num_days_from_monday() + 1 == target_weekday
                    }
                    "monthly" => {
                        let target_day = settings.day_of_month.unwrap_or(1);
                        now.date_naive().day() == target_day.min(last_day_of_month(now.date_naive().year(), now.date_naive().month()))
                    }
                    _ => true,
                };

                if !is_right_day {
                    false
                } else {
                    let run_state = load_run_state(&db.app_data_dir);
                    !already_ran_this_period(&run_state, &now, &settings)
                }
            }
        };

        if should_run {
            let target_dir = settings.target_dir.as_ref().unwrap();

            let guard = db.maintenance_guard();
            match guard {
                Ok(guard) => {
                    let now_str = chrono::Local::now()
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string();
                    let mut run_state = load_run_state(&db.app_data_dir);
                    run_state.last_run_at = Some(now_str.clone());
                    run_state.last_auto_run_at = Some(now_str);

                    match do_backup_with_type(&db, target_dir, "auto").await {
                        Ok(path) => {
                            run_state.last_success_at = run_state.last_run_at.clone();
                            run_state.last_status = Some("success".into());
                            run_state.last_message = Some("自动备份成功".into());
                            run_state.last_backup_path = Some(path);

                            let backups =
                                crate::commands::backup_settings::scan_backup_dir_public(
                                    Path::new(target_dir),
                                );
                            let retention_result = apply_retention(&settings, &backups);
                            if !retention_result.warnings.is_empty() {
                                run_state.last_message = Some(format!(
                                    "自动备份成功，清理旧备份时有警告: {}",
                                    retention_result.warnings.join("; ")
                                ));
                            }
                        }
                        Err(e) => {
                            run_state.last_status = Some("failed".into());
                            run_state.last_message = Some(format!("自动备份失败: {}", e));
                        }
                    }

                    let _ = save_run_state(&db.app_data_dir, &run_state);
                    drop(guard);
                }
                Err(_) => {
                    let mut run_state = load_run_state(&db.app_data_dir);
                    run_state.last_run_at = Some(
                        chrono::Local::now()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string(),
                    );
                    run_state.last_status = Some("skipped".into());
                    run_state.last_message =
                        Some("系统维护中，跳过本次自动备份".into());
                    let _ = save_run_state(&db.app_data_dir, &run_state);
                }
            }
        }

        // Calculate sleep duration
        let sleep_duration = if should_run {
            std::time::Duration::from_secs(60)
        } else if is_interval {
            // For intervals, sleep for a fraction of the interval to stay responsive
            let interval_secs = (settings.interval_minutes.unwrap_or(60) as u64) * 60;
            let sleep_secs = (interval_secs / 4).max(30).min(300);
            std::time::Duration::from_secs(sleep_secs)
        } else {
            // Time-of-day based: compute sleep until next target
            let (hour, minute) = parse_time(&settings.time_of_day).unwrap_or((23, 0));
            let next_target = match settings.frequency.as_str() {
                "weekly" => {
                    let target_weekday = settings.day_of_week.unwrap_or(1);
                    let today_weekday = now.date_naive().weekday().num_days_from_monday() + 1;
                    let today_target = now.date_naive().and_hms_opt(hour, minute, 0).unwrap();
                    let today_target_dt = chrono::Local.from_local_datetime(&today_target).single().unwrap();
                    if now < today_target_dt && today_weekday == target_weekday {
                        Some(today_target_dt)
                    } else {
                        let days_until = if today_weekday < target_weekday {
                            (target_weekday - today_weekday) as i64
                        } else {
                            (7 - today_weekday + target_weekday) as i64
                        };
                        let target_date = now.date_naive() + chrono::Duration::days(days_until);
                        target_date.and_hms_opt(hour, minute, 0)
                            .and_then(|t| chrono::Local.from_local_datetime(&t).single())
                    }
                }
                "monthly" => {
                    let target_day = settings.day_of_month.unwrap_or(1);
                    let today = now.date_naive();
                    let today_target = today.and_hms_opt(hour, minute, 0).unwrap();
                    let today_target_dt = chrono::Local.from_local_datetime(&today_target).single().unwrap();
                    let day_this_month = target_day.min(last_day_of_month(today.year(), today.month()));
                    if now < today_target_dt && today.day() == day_this_month {
                        Some(today_target_dt)
                    } else {
                        let (y, m) = if today.month() == 12 {
                            (today.year() + 1, 1)
                        } else {
                            (today.year(), today.month() + 1)
                        };
                        let day = target_day.min(last_day_of_month(y, m));
                        chrono::NaiveDate::from_ymd_opt(y, m, day)
                            .and_then(|d| d.and_hms_opt(hour, minute, 0))
                            .and_then(|t| chrono::Local.from_local_datetime(&t).single())
                    }
                }
                _ => {
                    let today_target = now.date_naive().and_hms_opt(hour, minute, 0).unwrap();
                    let today_target_dt = chrono::Local.from_local_datetime(&today_target).single().unwrap();
                    if now < today_target_dt {
                        Some(today_target_dt)
                    } else {
                        let tomorrow = now.date_naive() + chrono::Duration::days(1);
                        tomorrow.and_hms_opt(hour, minute, 0)
                            .and_then(|t| chrono::Local.from_local_datetime(&t).single())
                    }
                }
            };
            let duration = next_target
                .map(|t| (t - now).to_std().unwrap_or(std::time::Duration::from_secs(60)))
                .unwrap_or(std::time::Duration::from_secs(60));
            duration.min(std::time::Duration::from_secs(300))
        };

        tokio::select! {
            _ = tokio::time::sleep(sleep_duration) => {}
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    break;
                }
                continue;
            }
            _ = settings_rx.changed() => {
                continue;
            }
        }
    }
}
