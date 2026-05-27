use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub async fn backup_database(
    db: State<'_, DbState>,
    target_dir: String,
) -> Result<String, String> {
    // WAL checkpoint to flush to main DB file
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&db.pool)
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
    backup_path: String,
) -> Result<(), String> {
    let backup = std::path::Path::new(&backup_path);
    if !backup.exists() {
        return Err("备份文件不存在".into());
    }

    // Validate it's a SQLite file by checking header
    let header = std::fs::read(backup).map_err(|e| e.to_string())?;
    if header.len() < 16 || &header[0..16] != b"SQLite format 3\0" {
        return Err("无效的数据库备份文件".into());
    }

    // Restore: copy backup to current DB location
    let db_path = db.app_data_dir.join("finledger.db");

    // Close current pool by checkpointing and then replacing
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    // Copy backup over the current database
    std::fs::copy(backup, &db_path).map_err(|e| {
        format!("恢复失败: {}", e)
    })?;

    Ok(())
}
