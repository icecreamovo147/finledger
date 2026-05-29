use crate::commands::backup::{do_backup_with_type, do_restore};
use crate::commands::backup_scheduler::BackupSchedulerState;
use crate::db::DbState;
use crate::models::{BackupFileInfo, BackupManifest, BackupOverview, BackupRunState, BackupSettings};
use chrono::{Datelike, TimeZone};
use std::path::{Path, PathBuf};
use tauri::State;

fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("backup_settings.json")
}

fn run_state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("backup_run_state.json")
}

pub fn get_backup_settings_sync(db: &DbState) -> BackupSettings {
    load_settings(&db.app_data_dir)
}

fn load_settings(app_data_dir: &Path) -> BackupSettings {
    let path = settings_path(app_data_dir);
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<BackupSettings>(&data) {
                return settings;
            }
        }
    }
    BackupSettings::default()
}

pub fn save_settings(app_data_dir: &Path, settings: &BackupSettings) -> Result<(), String> {
    let path = settings_path(app_data_dir);
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("保存备份配置失败: {}", e))
}

pub fn load_run_state(app_data_dir: &Path) -> BackupRunState {
    let path = run_state_path(app_data_dir);
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<BackupRunState>(&data) {
                return state;
            }
        }
    }
    BackupRunState::default()
}

pub fn save_run_state(app_data_dir: &Path, state: &BackupRunState) -> Result<(), String> {
    let path = run_state_path(app_data_dir);
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("保存备份运行状态失败: {}", e))
}

fn read_manifest_from_zip(zip_path: &Path) -> Result<BackupManifest, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("读取zip失败: {}", e))?;
    let mut manifest_file = archive
        .by_name("manifest.json")
        .map_err(|e| format!("找不到manifest.json: {}", e))?;
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut manifest_file, &mut contents)
        .map_err(|e| format!("读取manifest失败: {}", e))?;
    serde_json::from_str(&contents).map_err(|e| format!("解析manifest失败: {}", e))
}

fn scan_backup_dir(target_dir: &Path) -> Vec<BackupFileInfo> {
    scan_backup_dir_public(target_dir)
}

pub fn scan_backup_dir_public(target_dir: &Path) -> Vec<BackupFileInfo> {
    let mut backups = Vec::new();
    let entries = match std::fs::read_dir(target_dir) {
        Ok(e) => e,
        Err(_) => return backups,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("flbackup") => {}
            _ => continue,
        }

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let size_bytes = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        match read_manifest_from_zip(&path) {
            Ok(manifest) => {
                backups.push(BackupFileInfo {
                    file_name,
                    path: path.to_string_lossy().to_string(),
                    backup_type: manifest.backup_type.unwrap_or_else(|| "unknown".into()),
                    created_at: Some(manifest.created_at),
                    size_bytes,
                    format_version: Some(manifest.backup_format_version),
                    images_count: Some(manifest.images_count),
                    is_valid: true,
                    validation_message: None,
                });
            }
            Err(e) => {
                backups.push(BackupFileInfo {
                    file_name,
                    path: path.to_string_lossy().to_string(),
                    backup_type: "unknown".into(),
                    created_at: None,
                    size_bytes,
                    format_version: None,
                    images_count: None,
                    is_valid: false,
                    validation_message: Some(e),
                });
            }
        }
    }

    // Sort by created_at descending, invalid ones go to the end
    backups.sort_by(|a, b| {
        match (&b.created_at, &a.created_at) {
            (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    backups
}

fn next_daily(now: chrono::DateTime<chrono::Local>, hour: u32, minute: u32) -> Option<chrono::DateTime<chrono::Local>> {
    let today_target = now
        .date_naive()
        .and_hms_opt(hour, minute, 0)?;
    let today_target = chrono::Local.from_local_datetime(&today_target).single()?;
    if now < today_target {
        Some(today_target)
    } else {
        let tomorrow = now.date_naive() + chrono::Duration::days(1);
        let t = tomorrow.and_hms_opt(hour, minute, 0)?;
        chrono::Local.from_local_datetime(&t).single()
    }
}

fn next_weekly(
    now: chrono::DateTime<chrono::Local>,
    hour: u32,
    minute: u32,
    day_of_week: u32,
) -> Option<chrono::DateTime<chrono::Local>> {
    let today_weekday = now.date_naive().weekday().num_days_from_monday() + 1; // 1=Mon..7=Sun
    let target_weekday = day_of_week; // 1=Mon..7=Sun

    let days_until = if today_weekday < target_weekday {
        target_weekday - today_weekday
    } else if today_weekday == target_weekday {
        // Same day — check if time has passed
        let today_target = now.date_naive().and_hms_opt(hour, minute, 0)?;
        let today_target = chrono::Local.from_local_datetime(&today_target).single()?;
        if now < today_target {
            return Some(today_target);
        }
        7
    } else {
        7 - (today_weekday - target_weekday)
    };

    let target_date = now.date_naive() + chrono::Duration::days(days_until as i64);
    let t = target_date.and_hms_opt(hour, minute, 0)?;
    chrono::Local.from_local_datetime(&t).single()
}

fn next_monthly(
    now: chrono::DateTime<chrono::Local>,
    hour: u32,
    minute: u32,
    day_of_month: u32,
) -> Option<chrono::DateTime<chrono::Local>> {
    let today = now.date_naive();
    let current_month = today.year();
    let current_month_num = today.month();

    // Try this month first
    let target_day = day_of_month.min(last_day_of_month(current_month, current_month_num));
    if let Some(target_date) = chrono::NaiveDate::from_ymd_opt(current_month, current_month_num, target_day) {
        if target_date >= today {
            let t = target_date.and_hms_opt(hour, minute, 0)?;
            let dt = chrono::Local.from_local_datetime(&t).single()?;
            if today == target_date && now >= dt {
                // Already past today, try next month
            } else {
                return Some(dt);
            }
        }
    }

    // Try next month
    let (next_year, next_month) = if current_month_num == 12 {
        (current_month + 1, 1)
    } else {
        (current_month, current_month_num + 1)
    };
    let target_day = day_of_month.min(last_day_of_month(next_year, next_month));
    let target_date = chrono::NaiveDate::from_ymd_opt(next_year, next_month, target_day)?;
    let t = target_date.and_hms_opt(hour, minute, 0)?;
    chrono::Local.from_local_datetime(&t).single()
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

fn compute_next_backup_at(settings: &BackupSettings, run_state: &BackupRunState) -> Option<String> {
    if !settings.enabled || settings.target_dir.is_none() {
        return None;
    }

    let now = chrono::Local::now();

    let next = match settings.frequency.as_str() {
        "interval_minutes" | "interval_hours" => {
            let interval = settings.interval_minutes.unwrap_or(60) as i64;
            // Use last_auto_run_at — manual backups don't affect auto schedule
            let base = run_state
                .last_auto_run_at
                .as_ref()
                .and_then(|t| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S").ok())
                .and_then(|dt| chrono::Local.from_local_datetime(&dt).single());
            match base {
                Some(last) => {
                    let next = last + chrono::Duration::minutes(interval);
                    if next <= now { now } else { next }
                }
                None => {
                    // Never ran — first execution will happen almost immediately (after scheduler startup delay)
                    now
                }
            }
        }
        _ => {
            let parts: Vec<&str> = settings.time_of_day.split(':').collect();
            if parts.len() != 2 {
                return None;
            }
            let hour: u32 = parts[0].parse().ok()?;
            let minute: u32 = parts[1].parse().ok()?;

            match settings.frequency.as_str() {
                "weekly" => next_weekly(now, hour, minute, settings.day_of_week.unwrap_or(1))?,
                "monthly" => next_monthly(now, hour, minute, settings.day_of_month.unwrap_or(1))?,
                _ => next_daily(now, hour, minute)?,
            }
        }
    };

    Some(next.format("%Y-%m-%d %H:%M:%S").to_string())
}

pub struct RetentionResult {
    pub deleted_count: usize,
    pub failed_count: usize,
    pub warnings: Vec<String>,
}

pub fn apply_retention(
    settings: &BackupSettings,
    backups: &[BackupFileInfo],
) -> RetentionResult {
    let mut result = RetentionResult {
        deleted_count: 0,
        failed_count: 0,
        warnings: Vec::new(),
    };

    if settings.retention_mode != "count" {
        return result;
    }

    let target_dir = match &settings.target_dir {
        Some(d) => Path::new(d),
        None => return result,
    };

    // Filter auto backups that are valid and in the configured directory
    let mut auto_backups: Vec<&BackupFileInfo> = backups
        .iter()
        .filter(|b| {
            b.backup_type == "auto"
                && b.is_valid
                && Path::new(&b.path).parent() == Some(target_dir)
        })
        .collect();

    // Already sorted by created_at descending from scan_backup_dir
    // Keep the newest retention_count, delete the rest
    if auto_backups.len() <= settings.retention_count as usize {
        return result;
    }

    let to_delete = auto_backups.split_off(settings.retention_count as usize);
    for backup in to_delete {
        match std::fs::remove_file(&backup.path) {
            Ok(()) => result.deleted_count += 1,
            Err(e) => {
                result.failed_count += 1;
                result
                    .warnings
                    .push(format!("删除 {} 失败: {}", backup.file_name, e));
            }
        }
    }

    result
}

#[tauri::command]
pub async fn get_backup_settings(
    db: State<'_, DbState>,
    token: String,
) -> Result<BackupSettings, String> {
    db.validate_token(&token).await?;
    Ok(load_settings(&db.app_data_dir))
}

#[tauri::command]
pub async fn update_backup_settings(
    db: State<'_, DbState>,
    scheduler: State<'_, BackupSchedulerState>,
    token: String,
    settings: BackupSettings,
) -> Result<BackupSettings, String> {
    db.validate_token(&token).await?;

    // Validate target_dir if provided
    if settings.enabled {
        if let Some(ref dir) = settings.target_dir {
            let path = Path::new(dir);
            if !path.exists() {
                return Err("备份目录不存在".into());
            }
            if !path.is_dir() {
                return Err("备份路径不是目录".into());
            }
        } else {
            return Err("请先选择备份目录".into());
        }

        // Validate frequency-specific fields
        match settings.frequency.as_str() {
            "interval_minutes" | "interval_hours" => {
                match settings.interval_minutes {
                    None | Some(0) => return Err("请设置备份间隔".into()),
                    Some(m) if m > 10080 => return Err("备份间隔不能超过 7 天".into()),
                    _ => {}
                }
            }
            "weekly" => {
                match settings.day_of_week {
                    None => return Err("请选择每周执行日".into()),
                    Some(d) if d < 1 || d > 7 => return Err("每周执行日必须在 1-7 之间".into()),
                    _ => {}
                }
            }
            "monthly" => {
                match settings.day_of_month {
                    None => return Err("请选择每月执行日".into()),
                    Some(d) if d < 1 || d > 28 => return Err("每月执行日必须在 1-28 之间".into()),
                    _ => {}
                }
            }
            "daily" => {}
            other => return Err(format!("不支持的备份频率: {}", other)),
        }
    }

    save_settings(&db.app_data_dir, &settings)?;

    // Restart scheduler with new settings
    scheduler.restart(&db, settings.clone()).await;

    Ok(settings)
}

#[tauri::command]
pub async fn get_backup_overview(
    db: State<'_, DbState>,
    token: String,
) -> Result<BackupOverview, String> {
    db.validate_token(&token).await?;
    let settings = load_settings(&db.app_data_dir);
    let run_state = load_run_state(&db.app_data_dir);

    let (backups, total_count, auto_count, manual_count, unknown_count, total_size_bytes, oldest, latest) =
        if let Some(ref dir) = settings.target_dir {
            let files = scan_backup_dir(Path::new(dir));
            let total = files.len();
            let auto = files.iter().filter(|b| b.backup_type == "auto").count();
            let manual = files.iter().filter(|b| b.backup_type == "manual").count();
            let unknown = total - auto - manual;
            let size: u64 = files.iter().map(|b| b.size_bytes).sum();
            let oldest = files.last().and_then(|b| b.created_at.clone());
            let latest = files.first().and_then(|b| b.created_at.clone());
            (files, total, auto, manual, unknown, size, oldest, latest)
        } else {
            (Vec::new(), 0, 0, 0, 0, 0, None, None)
        };

    let next_backup_at = compute_next_backup_at(&settings, &run_state);

    Ok(BackupOverview {
        settings,
        total_count,
        auto_count,
        manual_count,
        unknown_count,
        total_size_bytes,
        oldest_backup_at: oldest,
        latest_backup_at: latest,
        last_run_state: run_state,
        next_backup_at,
        backups,
    })
}

#[tauri::command]
pub async fn run_backup_now(
    db: State<'_, DbState>,
    token: String,
    backup_type: Option<String>,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    let settings = load_settings(&db.app_data_dir);
    let target_dir = settings
        .target_dir
        .as_deref()
        .ok_or("请先配置备份目录")?;

    let btype = backup_type.as_deref().unwrap_or("manual");

    let _guard = db.maintenance_guard()?;
    let result = do_backup_with_type(&db, target_dir, btype).await;

    // Update run state
    let now_str = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let mut run_state = load_run_state(&db.app_data_dir);
    run_state.last_run_at = Some(now_str.clone());
    if btype == "auto" {
        run_state.last_auto_run_at = Some(now_str.clone());
    }

    match &result {
        Ok(path) => {
            run_state.last_success_at = Some(now_str);
            run_state.last_status = Some("success".into());
            run_state.last_message = Some("备份成功".into());
            run_state.last_backup_path = Some(path.clone());

            // Apply retention after successful auto backup
            if btype == "auto" {
                let overview_backups = scan_backup_dir(Path::new(target_dir));
                let retention_result = apply_retention(&settings, &overview_backups);
                if !retention_result.warnings.is_empty() {
                    run_state.last_message = Some(format!(
                        "备份成功，但清理旧备份时有警告: {}",
                        retention_result.warnings.join("; ")
                    ));
                }
            }
        }
        Err(e) => {
            run_state.last_status = Some("failed".into());
            run_state.last_message = Some(e.clone());
        }
    }

    let _ = save_run_state(&db.app_data_dir, &run_state);

    result
}

#[tauri::command]
pub async fn restore_backup(
    db: State<'_, DbState>,
    token: String,
    backup_path: String,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    let _guard = db.maintenance_guard()?;
    do_restore(&db, &backup_path).await
}

#[tauri::command]
pub async fn delete_backup_file(
    db: State<'_, DbState>,
    token: String,
    path: String,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    let file_path = Path::new(&path);

    // Security: must be a file, not a directory
    if !file_path.is_file() {
        return Err("只能删除备份文件".into());
    }

    // Security: must have .flbackup extension
    match file_path.extension().and_then(|e| e.to_str()) {
        Some("flbackup") => {}
        _ => return Err("只能删除 .flbackup 文件".into()),
    }

    // Security: must be under the configured backup directory
    let settings = load_settings(&db.app_data_dir);
    if let Some(ref dir) = settings.target_dir {
        let target_dir = Path::new(dir);
        if let Ok(canonical_file) = file_path.canonicalize() {
            if let Ok(canonical_dir) = target_dir.canonicalize() {
                if !canonical_file.starts_with(&canonical_dir) {
                    return Err("只能删除备份目录内的文件".into());
                }
            } else {
                return Err("备份目录无效".into());
            }
        } else {
            return Err("文件路径无效".into());
        }
    } else {
        return Err("未配置备份目录".into());
    }

    std::fs::remove_file(file_path).map_err(|e| format!("删除失败: {}", e))
}
