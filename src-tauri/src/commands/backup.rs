use crate::db::DbState;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tauri::State;

#[tauri::command]
pub async fn backup_database(
    db: State<'_, DbState>,
    token: String,
    target_dir: String,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    // Pre-backup integrity check — warn but don't block
    let check: Vec<(String,)> = sqlx::query_as("PRAGMA integrity_check")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if !check.iter().all(|(v,)| v == "ok") {
        return Err("数据库完整性检测异常，建议先修复再备份。请前往设置页恢复备份或检查数据库。".into());
    }

    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let db_path = db.app_data_dir.join("finledger.db");
    if !db_path.exists() {
        return Err("数据库文件不存在".into());
    }

    let filename = format!(
        "finledger_backup_{}.db",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let target_path = std::path::Path::new(&target_dir).join(&filename);

    std::fs::copy(&db_path, &target_path).map_err(|e| {
        format!("备份失败: {}", e)
    })?;

    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn restore_database(
    db: State<'_, DbState>,
    token: String,
    backup_path: String,
) -> Result<(), String> {
    db.validate_token(&token).await?;

    // Acquire maintenance lock to block all other DB commands
    if !db.begin_maintenance() {
        return Err("系统维护中，请稍后再试".into());
    }

    let result = do_restore(&db, &backup_path).await;

    db.end_maintenance();
    result
}

async fn do_restore(db: &DbState, backup_path: &str) -> Result<(), String> {
    let backup = std::path::Path::new(backup_path);
    if !backup.exists() {
        return Err("备份文件不存在".into());
    }

    let header = std::fs::read(backup).map_err(|e| e.to_string())?;
    if header.len() < 16 || &header[0..16] != b"SQLite format 3\0" {
        return Err("无效的数据库备份文件".into());
    }

    let db_path = db.app_data_dir.join("finledger.db");
    let wal_path = db_path.with_extension("db-wal");
    let shm_path = db_path.with_extension("db-shm");

    // Back up the current database for rollback
    let backup_current = db_path.with_extension("db.pre-restore");
    let backup_wal = wal_path.with_extension("db-wal.pre-restore");
    let backup_shm = shm_path.with_extension("db-shm.pre-restore");

    std::fs::copy(&db_path, &backup_current).map_err(|e| format!("备份当前数据库失败: {}", e))?;
    if wal_path.exists() {
        std::fs::copy(&wal_path, &backup_wal).ok();
    }
    if shm_path.exists() {
        std::fs::copy(&shm_path, &backup_shm).ok();
    }

    // Close the current pool (maintenance lock prevents new commands from acquiring it)
    {
        let old_pool = db.raw_pool().await;
        old_pool.close().await;
    }

    // Remove WAL and SHM files
    std::fs::remove_file(&wal_path).ok();
    std::fs::remove_file(&shm_path).ok();

    // Copy backup over the current database
    if let Err(e) = std::fs::copy(backup, &db_path) {
        rollback(&db, &db_path, &wal_path, &shm_path, &backup_current, &backup_wal, &backup_shm).await;
        return Err(format!("恢复失败，已回滚: {}", e));
    }

    // Create a new pool from the restored database
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let new_pool = match SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            SqliteConnectOptions::from_str(&db_url)
                .map_err(|e| e.to_string())?
                .foreign_keys(true),
        )
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            rollback(&db, &db_path, &wal_path, &shm_path, &backup_current, &backup_wal, &backup_shm).await;
            return Err(format!("恢复后重连失败，已回滚: {}", e));
        }
    };

    db.replace_pool(new_pool).await;

    // Post-restore integrity check
    if let Some(err) = db.check_integrity().await {
        eprintln!("恢复后完整性检测: {}", err);
    }

    // Clean up temp backup files
    std::fs::remove_file(&backup_current).ok();
    std::fs::remove_file(&backup_wal).ok();
    std::fs::remove_file(&backup_shm).ok();

    Ok(())
}

async fn rollback(
    db: &DbState,
    db_path: &std::path::Path,
    wal_path: &std::path::Path,
    shm_path: &std::path::Path,
    backup_current: &std::path::Path,
    backup_wal: &std::path::Path,
    backup_shm: &std::path::Path,
) {
    std::fs::copy(backup_current, db_path).ok();
    if backup_wal.exists() {
        std::fs::copy(backup_wal, wal_path).ok();
    }
    if backup_shm.exists() {
        std::fs::copy(backup_shm, shm_path).ok();
    }
    std::fs::remove_file(backup_current).ok();
    std::fs::remove_file(backup_wal).ok();
    std::fs::remove_file(backup_shm).ok();

    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    if let Ok(rollback_pool) = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            SqliteConnectOptions::from_str(&db_url)
                .unwrap()
                .foreign_keys(true),
        )
        .await
    {
        db.replace_pool(rollback_pool).await;
    }
}
