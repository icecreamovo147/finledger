use crate::db::{sqlite_options, DbState};
use crate::models::BackupManifest;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};
use sqlx::sqlite::SqlitePoolOptions;
use std::io::{Read as IoRead, Write};
use std::path::Path;
use tauri::State;
use zip::write::SimpleFileOptions;

fn compute_sha256(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_zip_file(path: &Path) -> bool {
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut magic = [0u8; 4];
        if f.read_exact(&mut magic).is_ok() {
            return magic == [0x50, 0x4B, 0x03, 0x04];
        }
    }
    false
}

fn is_sqlite_file(path: &Path) -> bool {
    if let Ok(data) = std::fs::read(path) {
        data.len() >= 16 && &data[0..16] == b"SQLite format 3\0"
    } else {
        false
    }
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base_dir: &Path,
    prefix: &str,
) -> Result<u32, String> {
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| format!("无法解析图片目录: {}", e))?;
    let mut count = 0u32;
    let entries = std::fs::read_dir(base_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = path
            .file_name()
            .ok_or("invalid file name")?
            .to_string_lossy();
        let zip_path = format!("{}/{}", prefix, name);

        if path.is_dir() {
            let canonical_sub = path
                .canonicalize()
                .map_err(|e| format!("无法解析子目录: {}", e))?;
            if !canonical_sub.starts_with(&canonical_base) {
                warn!("警告: 跳过不在备份目录内的目录: {}", canonical_sub.display());
                continue;
            }
            count += add_dir_to_zip(zip, &path, &zip_path)?;
        } else {
            // Verify the file is within the canonical base directory
            let canonical_path = path
                .canonicalize()
                .map_err(|e| format!("无法解析文件路径: {}", e))?;
            if !canonical_path.starts_with(&canonical_base) {
                warn!("警告: 跳过不在备份目录内的文件: {}", canonical_path.display());
                continue;
            }
            zip.start_file(&zip_path, SimpleFileOptions::default())
                .map_err(|e| e.to_string())?;
            let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}

#[tauri::command]
pub async fn backup_database(
    db: State<'_, DbState>,
    token: String,
    target_dir: String,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    let _guard = db.maintenance_guard()?;
    do_backup_with_type(&db, &target_dir, "manual").await
}

#[tauri::command]
pub async fn restore_database(
    db: State<'_, DbState>,
    token: String,
    backup_path: String,
) -> Result<String, String> {
    db.validate_token(&token).await?;

    let _guard = db.maintenance_guard()?;

    let backup = Path::new(&backup_path);
    if !backup.exists() {
        return Err("备份文件不存在".into());
    }

    if is_zip_file(backup) {
        // New .flbackup format
        do_restore_flbackup(&db, backup).await
    } else if is_sqlite_file(backup) {
        // Legacy .db format — guard auto-releases on return
        do_restore_legacy_db(&db, backup).await
    } else {
        Err("无效的备份文件格式".into())
    }
}

async fn do_restore_flbackup(db: &DbState, backup: &Path) -> Result<String, String> {
    let tmp_id = uuid::Uuid::new_v4().to_string();
    let tmp_dir = db.app_data_dir.join(format!(".restore-tmp-{}", tmp_id));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    // Extract zip
    let zip_file = std::fs::File::open(backup).map_err(|e| format!("打开备份文件失败: {}", e))?;
    let mut archive = zip::ZipArchive::new(zip_file).map_err(|e| format!("解压失败: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = match file.enclosed_name() {
            Some(path) => tmp_dir.join(path),
            None => continue,
        };
        if file.is_dir() {
            std::fs::create_dir_all(&outpath).ok();
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p).ok();
            }
            let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    // Parse and validate manifest
    let manifest_path = tmp_dir.join("manifest.json");
    let extracted_db = tmp_dir.join("finledger.db");
    let extracted_images = tmp_dir.join("images");

    if manifest_path.exists() && extracted_db.exists() {
        let manifest_data = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        let manifest: BackupManifest = serde_json::from_str(&manifest_data)
            .map_err(|e| format!("解析 manifest 失败: {}", e))?;

        let actual_sha256 = compute_sha256(&extracted_db)?;
        if actual_sha256 != manifest.db_sha256 {
            cleanup_dir(&tmp_dir);
            return Err("备份文件已损坏，校验和不匹配".into());
        }
    } else if !extracted_db.exists() {
        cleanup_dir(&tmp_dir);
        return Err("备份文件中缺少数据库".into());
    }

    // Perform the restore
    let result = restore_db_and_images(
        db,
        &extracted_db,
        if extracted_images.exists() {
            Some(&extracted_images)
        } else {
            None
        },
    )
    .await;

    cleanup_dir(&tmp_dir);

    match result {
        Ok(()) => Ok("数据恢复成功，包含数据库和附件图片".into()),
        Err(e) => Err(e),
    }
}

async fn do_restore_legacy_db(db: &DbState, backup: &Path) -> Result<String, String> {
    let tmp_id = uuid::Uuid::new_v4().to_string();
    let tmp_dir = db.app_data_dir.join(format!(".restore-tmp-{}", tmp_id));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    let tmp_db = tmp_dir.join("finledger.db");
    std::fs::copy(backup, &tmp_db).map_err(|e| format!("复制备份失败: {}", e))?;

    let result = restore_db_and_images(db, &tmp_db, None).await;

    cleanup_dir(&tmp_dir);

    match result {
        Ok(()) => Ok("数据恢复成功（旧版备份，不包含图片附件）。建议创建新的完整备份。".into()),
        Err(e) => Err(e),
    }
}

async fn restore_db_and_images(
    db: &DbState,
    new_db_path: &Path,
    new_images_dir: Option<&Path>,
) -> Result<(), String> {
    let db_path = db.app_data_dir.join("finledger.db");
    let wal_path = db_path.with_extension("db-wal");
    let shm_path = db_path.with_extension("db-shm");

    info!("开始恢复数据库 from={}", new_db_path.display());
    // Back up current DB for rollback
    let backup_current = db_path.with_extension("db.pre-restore");
    let backup_wal = wal_path.with_extension("db-wal.pre-restore");
    let backup_shm = shm_path.with_extension("db-shm.pre-restore");

    std::fs::copy(&db_path, &backup_current).map_err(|e| format!("备份当前数据库失败: {}", e))?;
    if wal_path.exists() {
        std::fs::copy(&wal_path, &backup_wal).map_err(|e| {
            let _ = std::fs::remove_file(&backup_current);
            format!("备份WAL文件失败: {}", e)
        })?;
    }
    if shm_path.exists() {
        std::fs::copy(&shm_path, &backup_shm).map_err(|e| {
            let _ = std::fs::remove_file(&backup_current);
            let _ = std::fs::remove_file(&backup_wal);
            format!("备份SHM文件失败: {}", e)
        })?;
    }

    // Back up current images directory for rollback
    let backup_images_dir = db.app_data_dir.join(".images-pre-restore");
    let has_images_backup = if db.images_dir.exists() {
        // Remove any stale backup first
        if backup_images_dir.exists() {
            std::fs::remove_dir_all(&backup_images_dir).ok();
        }
        // Image backup MUST succeed — if it fails, we cannot safely proceed
        if let Err(e) = copy_dir_recursive(&db.images_dir, &backup_images_dir) {
            std::fs::remove_file(&backup_current).ok();
            std::fs::remove_file(&backup_wal).ok();
            std::fs::remove_file(&backup_shm).ok();
            std::fs::remove_dir_all(&backup_images_dir).ok();
            return Err(format!("备份当前图片目录失败，恢复中止: {}", e));
        }
        true
    } else {
        false
    };

    // Close current pool
    {
        let old_pool = db.raw_pool().await;
        old_pool.close().await;
    }

    // Remove WAL and SHM files — must succeed before replacing DB
    if wal_path.exists() {
        if let Err(e) = std::fs::remove_file(&wal_path) {
            rollback_all(
                db,
                &db_path,
                &wal_path,
                &shm_path,
                &backup_current,
                &backup_wal,
                &backup_shm,
                &backup_images_dir,
                has_images_backup,
            )
            .await;
            return Err(format!("无法删除WAL文件，恢复中止: {}", e));
        }
    }
    if shm_path.exists() {
        if let Err(e) = std::fs::remove_file(&shm_path) {
            rollback_all(
                db,
                &db_path,
                &wal_path,
                &shm_path,
                &backup_current,
                &backup_wal,
                &backup_shm,
                &backup_images_dir,
                has_images_backup,
            )
            .await;
            return Err(format!("无法删除SHM文件，恢复中止: {}", e));
        }
    }

    // Replace database file atomically (temp + rename)
    let tmp_db = db_path.with_extension("db.restore_tmp");
    if let Err(e) = std::fs::copy(new_db_path, &tmp_db) {
        let _ = std::fs::remove_file(&tmp_db);
        rollback_all(
            db,
            &db_path,
            &wal_path,
            &shm_path,
            &backup_current,
            &backup_wal,
            &backup_shm,
            &backup_images_dir,
            has_images_backup,
        )
        .await;
        return Err(format!("恢复失败，已回滚: {}", e));
    }
    if let Err(e) = std::fs::rename(&tmp_db, &db_path) {
        let _ = std::fs::remove_file(&tmp_db);
        rollback_all(
            db,
            &db_path,
            &wal_path,
            &shm_path,
            &backup_current,
            &backup_wal,
            &backup_shm,
            &backup_images_dir,
            has_images_backup,
        )
        .await;
        return Err(format!("恢复失败，已回滚: {}", e));
    }

    // Replace images if provided — all failure paths must call rollback_all
    if let Some(img_dir) = new_images_dir {
        if db.images_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&db.images_dir) {
                rollback_all(
                    db,
                    &db_path,
                    &wal_path,
                    &shm_path,
                    &backup_current,
                    &backup_wal,
                    &backup_shm,
                    &backup_images_dir,
                    has_images_backup,
                )
                .await;
                return Err(format!("删除旧图片目录失败，已回滚: {}", e));
            }
        }
        if let Err(e) = std::fs::create_dir_all(&db.images_dir) {
            rollback_all(
                db,
                &db_path,
                &wal_path,
                &shm_path,
                &backup_current,
                &backup_wal,
                &backup_shm,
                &backup_images_dir,
                has_images_backup,
            )
            .await;
            return Err(format!("创建图片目录失败，已回滚: {}", e));
        }
        if let Err(e) = copy_dir_recursive(img_dir, &db.images_dir) {
            rollback_all(
                db,
                &db_path,
                &wal_path,
                &shm_path,
                &backup_current,
                &backup_wal,
                &backup_shm,
                &backup_images_dir,
                has_images_backup,
            )
            .await;
            return Err(format!("恢复图片失败，已回滚: {}", e));
        }
    }

    // Create new pool
    let new_pool = match SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(sqlite_options(&db_path))
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            rollback_all(
                db,
                &db_path,
                &wal_path,
                &shm_path,
                &backup_current,
                &backup_wal,
                &backup_shm,
                &backup_images_dir,
                has_images_backup,
            )
            .await;
            return Err(format!("恢复后重连失败，已回滚: {}", e));
        }
    };

    db.replace_pool(new_pool).await;

    // Post-restore integrity check — must fail on error
    if let Some(err) = db.check_integrity().await {
        rollback_all(
            db,
            &db_path,
            &wal_path,
            &shm_path,
            &backup_current,
            &backup_wal,
            &backup_shm,
            &backup_images_dir,
            has_images_backup,
        )
        .await;
        return Err(format!(
            "恢复后数据库完整性检测失败，已回滚到恢复前数据: {}",
            err
        ));
    }

    // Run migrations on the restored database
    if let Err(e) = db.run_migrations().await {
        rollback_all(
            db,
            &db_path,
            &wal_path,
            &shm_path,
            &backup_current,
            &backup_wal,
            &backup_shm,
            &backup_images_dir,
            has_images_backup,
        )
        .await;
        return Err(format!("恢复后迁移失败，已回滚: {}", e));
    }

    info!("数据库恢复成功");
    // Clean up rollback files only on success
    std::fs::remove_file(&backup_current).ok();
    std::fs::remove_file(&backup_wal).ok();
    std::fs::remove_file(&backup_shm).ok();
    if backup_images_dir.exists() {
        std::fs::remove_dir_all(&backup_images_dir).ok();
    }

    Ok(())
}

async fn rollback_all(
    db: &DbState,
    db_path: &Path,
    wal_path: &Path,
    shm_path: &Path,
    backup_current: &Path,
    backup_wal: &Path,
    backup_shm: &Path,
    backup_images_dir: &Path,
    has_images_backup: bool,
) {
    // Rollback database
    if let Err(e) = std::fs::copy(backup_current, db_path) {
        error!("回滚: 恢复数据库文件失败: {}", e);
    }
    if backup_wal.exists() {
        std::fs::copy(backup_wal, wal_path).ok();
    }
    if backup_shm.exists() {
        std::fs::copy(backup_shm, shm_path).ok();
    }
    std::fs::remove_file(backup_current).ok();
    std::fs::remove_file(backup_wal).ok();
    std::fs::remove_file(backup_shm).ok();

    // Rollback images directory
    if has_images_backup && backup_images_dir.exists() {
        if db.images_dir.exists() {
            std::fs::remove_dir_all(&db.images_dir).ok();
        }
        if let Err(e) = copy_dir_recursive(backup_images_dir, &db.images_dir) {
            error!("回滚: 恢复图片目录失败: {}", e);
        }
        std::fs::remove_dir_all(backup_images_dir).ok();
    }

    // Reconnect pool
    if let Ok(rollback_pool) = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(sqlite_options(&db_path))
        .await
    {
        db.replace_pool(rollback_pool).await;
    } else {
        error!("回滚: 重建连接池失败");
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    }
    let entries = std::fs::read_dir(src).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(src_path.file_name().ok_or("无法获取文件名")?);
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn count_files_in_dir(dir: &Path) -> u32 {
    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_in_dir(&path);
            } else {
                count += 1;
            }
        }
    }
    count
}

fn cleanup_dir(dir: &Path) {
    if dir.exists() {
        std::fs::remove_dir_all(dir).ok();
    }
}

fn backup_type_file_part(backup_type: &str) -> String {
    let safe: String = backup_type
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    if safe.is_empty() {
        "manual".into()
    } else {
        safe
    }
}

// ===== Public test-facing helpers (no Tauri State, no token) =====

pub async fn do_backup(db: &DbState, target_dir: &str) -> Result<String, String> {
    do_backup_with_type(db, target_dir, "manual").await
}

pub async fn do_backup_with_type(
    db: &DbState,
    target_dir: &str,
    backup_type: &str,
) -> Result<String, String> {
    let pool = db.raw_pool().await;

    let check: Vec<(String,)> = sqlx::query_as("PRAGMA integrity_check")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if !check.iter().all(|(v,)| v == "ok") {
        return Err("数据库完整性检测异常".into());
    }

    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    info!("开始备份 type={} target={}", backup_type, target_dir);

    let db_path = db.app_data_dir.join("finledger.db");
    if !db_path.exists() {
        return Err("数据库文件不存在".into());
    }

    let tmp_id = uuid::Uuid::new_v4().to_string();
    let tmp_dir = db.app_data_dir.join(format!(".backup-tmp-{}", tmp_id));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    let tmp_db = tmp_dir.join("finledger.db");
    std::fs::copy(&db_path, &tmp_db).map_err(|e| {
        cleanup_dir(&tmp_dir);
        format!("复制数据库失败: {}", e)
    })?;

    let tmp_images = tmp_dir.join("images");
    let images_count = if db.images_dir.exists() {
        std::fs::create_dir_all(&tmp_images).map_err(|e| e.to_string())?;
        copy_dir_recursive(&db.images_dir, &tmp_images).map_err(|e| {
            cleanup_dir(&tmp_dir);
            e
        })?;
        count_files_in_dir(&tmp_images)
    } else {
        std::fs::create_dir_all(&tmp_images).ok();
        0
    };

    let db_sha256 = compute_sha256(&tmp_db).map_err(|e| {
        cleanup_dir(&tmp_dir);
        e
    })?;

    let manifest = BackupManifest {
        backup_format_version: 1,
        app: "FinLedger".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        db_sha256,
        images_count,
        backup_type: Some(backup_type.to_string()),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
        cleanup_dir(&tmp_dir);
        e.to_string()
    })?;
    let manifest_path = tmp_dir.join("manifest.json");
    std::fs::write(&manifest_path, &manifest_json).map_err(|e| {
        cleanup_dir(&tmp_dir);
        format!("写入 manifest 失败: {}", e)
    })?;

    let backup_type_for_file = backup_type_file_part(backup_type);
    let filename = format!(
        "finledger_{}_backup_{}.flbackup",
        backup_type_for_file,
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let zip_tmp_path = tmp_dir.join(&filename);
    let zip_file = std::fs::File::create(&zip_tmp_path).map_err(|e| {
        cleanup_dir(&tmp_dir);
        format!("创建归档文件失败: {}", e)
    })?;
    let mut zip_writer = zip::ZipWriter::new(zip_file);

    zip_writer
        .start_file("finledger.db", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    let mut db_file = std::fs::File::open(&tmp_db).map_err(|e| e.to_string())?;
    std::io::copy(&mut db_file, &mut zip_writer).map_err(|e| e.to_string())?;

    if db.images_dir.exists() {
        add_dir_to_zip(&mut zip_writer, &db.images_dir, "images").map_err(|e| {
            cleanup_dir(&tmp_dir);
            e
        })?;
    }

    zip_writer
        .start_file("manifest.json", SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip_writer
        .write_all(manifest_json.as_bytes())
        .map_err(|e| e.to_string())?;

    zip_writer.finish().map_err(|e| {
        cleanup_dir(&tmp_dir);
        format!("完成归档失败: {}", e)
    })?;

    let target_path = Path::new(target_dir).join(&filename);
    if let Err(e) = std::fs::rename(&zip_tmp_path, &target_path) {
        if let Err(copy_err) = std::fs::copy(&zip_tmp_path, &target_path) {
            cleanup_dir(&tmp_dir);
            return Err(format!("移动备份文件失败: rename={}, copy={}", e, copy_err));
        }
        std::fs::remove_file(&zip_tmp_path).ok();
    }

    cleanup_dir(&tmp_dir);
    info!("备份完成: {}", target_path.display());
    Ok(target_path.to_string_lossy().to_string())
}

pub async fn do_restore(db: &DbState, backup_path: &str) -> Result<String, String> {
    let backup = Path::new(backup_path);
    if !backup.exists() {
        return Err("备份文件不存在".into());
    }

    if is_zip_file(backup) {
        do_restore_flbackup(db, backup).await
    } else if is_sqlite_file(backup) {
        do_restore_legacy_db(db, backup).await
    } else {
        Err("无效的备份文件格式".into())
    }
}
