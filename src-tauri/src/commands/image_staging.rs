use crate::commands::record::{bytes_to_data_url, resolve_image_path};
use crate::db::DbState;
use crate::models::{IncomeImage, IncomeRecord};
use std::path::{Path, PathBuf};
use tauri::State;
use tracing::{info, warn};
use uuid::Uuid;

const IMAGE_SIZE_LIMIT: usize = 20 * 1024 * 1024;

const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp"];

fn validate_image_extension(file_name: &str) -> Result<&str, String> {
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let lower = ext.to_lowercase();
    if ALLOWED_IMAGE_EXTENSIONS.contains(&lower.as_str()) {
        Ok(ext)
    } else if ext.is_empty() {
        Err("无法识别图片格式，请选择常见图片文件".into())
    } else {
        Err(format!(
            "不支持的图片格式 .{}，支持：jpg, jpeg, png, gif, bmp, webp",
            ext
        ))
    }
}

#[derive(serde::Serialize)]
pub struct StagedImage {
    pub temp_id: String,
    pub original_name: String,
    pub preview_data_url: String,
}

fn staging_dir(db: &DbState) -> PathBuf {
    db.app_data_dir.join("temp-images")
}

fn sanitize_path_component(component: &str) -> Result<(), String> {
    if component.is_empty()
        || component.contains('/')
        || component.contains('\\')
        || component.contains("..")
        || Path::new(component).is_absolute()
    {
        return Err("非法的路径标识".into());
    }
    Ok(())
}

/// Validates session_id and returns the canonical session directory path.
/// Rejects empty, absolute, and traversal-containing identifiers.
fn resolve_session_dir(db: &DbState, session_id: &str) -> Result<PathBuf, String> {
    sanitize_path_component(session_id)?;
    let dir = staging_dir(db).join(session_id);
    Ok(dir)
}

fn session_manifest_path(db: &DbState, session_id: &str) -> Result<PathBuf, String> {
    resolve_session_dir(db, session_id).map(|dir| dir.join("manifest.json"))
}

fn load_session_manifest(
    db: &DbState,
    session_id: &str,
) -> std::collections::HashMap<String, String> {
    let path = match session_manifest_path(db, session_id) {
        Ok(p) => p,
        Err(_) => return std::collections::HashMap::new(),
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_session_manifest(
    db: &DbState,
    session_id: &str,
    manifest: &std::collections::HashMap<String, String>,
) {
    let path = match session_manifest_path(db, session_id) {
        Ok(p) => p,
        Err(_) => return,
    };
    if let Ok(json) = serde_json::to_string(manifest) {
        let tmp = path.with_extension("json.tmp");
        if let Err(e) = std::fs::write(&tmp, &json) {
            warn!("警告: 写入暂存 manifest 临时文件失败: {}", e);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            warn!("警告: 暂存 manifest 原子替换失败: {}", e);
            if let Err(e2) = std::fs::write(&path, &json) {
                warn!("警告: 暂存 manifest 兜底写入也失败: {}", e2);
            }
        }
    }
}

fn add_to_session_manifest(db: &DbState, session_id: &str, temp_id: &str, original_name: &str) {
    let mut manifest = load_session_manifest(db, session_id);
    manifest.insert(temp_id.to_string(), original_name.to_string());
    save_session_manifest(db, session_id, &manifest);
}

fn remove_from_session_manifest(db: &DbState, session_id: &str, temp_id: &str) {
    let mut manifest = load_session_manifest(db, session_id);
    if manifest.remove(temp_id).is_some() {
        save_session_manifest(db, session_id, &manifest);
    }
}

fn resolve_staged_path(
    db: &DbState,
    session_id: &str,
    temp_id: &str,
) -> Result<PathBuf, String> {
    sanitize_path_component(temp_id)?;

    let dir = resolve_session_dir(db, session_id)?;
    let full = dir.join(temp_id);

    // Canonicalize if it exists to verify boundary.
    // Canonicalize dir as well to handle symlinks (e.g. macOS /var → /private/var).
    if let Ok(canonical) = full.canonicalize() {
        let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if canonical.starts_with(&canonical_dir) {
            return Ok(canonical);
        }
        return Err("非法的暂存路径".into());
    }

    // File doesn't exist yet — still verify parent is the session dir
    Ok(full)
}

fn generate_temp_id(ext: &str) -> String {
    format!("{}.{}", Uuid::new_v4(), ext)
}

fn build_preview_data_url(bytes: &[u8], original_name: &str) -> String {
    bytes_to_data_url(bytes, original_name)
}

#[tauri::command]
pub async fn stage_image_from_path(
    db: State<'_, DbState>,
    token: String,
    session_id: String,
    path: String,
) -> Result<StagedImage, String> {
    db.validate_token(&token).await?;

    let src = PathBuf::from(&path);
    if !src.is_file() {
        return Err("请选择图片文件".into());
    }

    let original_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无法读取文件名".to_string())?
        .to_string();

    let ext = validate_image_extension(&original_name)?;

    let metadata = std::fs::metadata(&src).map_err(|_| "读取图片失败".to_string())?;
    if metadata.len() > IMAGE_SIZE_LIMIT as u64 {
        return Err("图片大小不能超过20MB".into());
    }

    let file_bytes = std::fs::read(&src).map_err(|_| "读取图片失败".to_string())?;
    let preview_data_url = build_preview_data_url(&file_bytes, &original_name);

    let dir = resolve_session_dir(&db, &session_id)?;
    std::fs::create_dir_all(&dir).map_err(|_| "暂存图片保存失败，请重试".to_string())?;

    let temp_id = generate_temp_id(ext);
    let dest = dir.join(&temp_id);
    std::fs::write(&dest, &file_bytes).map_err(|_| "暂存图片保存失败，请重试".to_string())?;

    add_to_session_manifest(&db, &session_id, &temp_id, &original_name);

    Ok(StagedImage {
        temp_id,
        original_name,
        preview_data_url,
    })
}

#[tauri::command]
pub async fn stage_image_bytes(
    db: State<'_, DbState>,
    token: String,
    session_id: String,
    file_name: String,
    file_bytes: Vec<u8>,
) -> Result<StagedImage, String> {
    db.validate_token(&token).await?;

    if file_bytes.len() > IMAGE_SIZE_LIMIT {
        return Err("图片大小不能超过20MB".into());
    }

    let ext = validate_image_extension(&file_name)?;
    let preview_data_url = build_preview_data_url(&file_bytes, &file_name);

    let dir = resolve_session_dir(&db, &session_id)?;
    std::fs::create_dir_all(&dir).map_err(|_| "暂存图片保存失败，请重试".to_string())?;

    let temp_id = generate_temp_id(ext);
    let dest = dir.join(&temp_id);
    std::fs::write(&dest, &file_bytes).map_err(|_| "暂存图片保存失败，请重试".to_string())?;

    add_to_session_manifest(&db, &session_id, &temp_id, &file_name);

    Ok(StagedImage {
        temp_id,
        original_name: file_name,
        preview_data_url,
    })
}

#[tauri::command]
pub async fn delete_staged_image(
    db: State<'_, DbState>,
    token: String,
    session_id: String,
    temp_id: String,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    let path = resolve_staged_path(&db, &session_id, &temp_id)?;
    if path.exists() {
        std::fs::remove_file(&path).ok();
    }
    // 从 manifest 中移除条目，避免累积脏数据
    remove_from_session_manifest(&db, &session_id, &temp_id);
    Ok(())
}

#[tauri::command]
pub async fn cancel_image_staging_session(
    db: State<'_, DbState>,
    token: String,
    session_id: String,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    let dir = resolve_session_dir(&db, &session_id)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).ok();
    }
    Ok(())
}

/// Remove staging session directories older than 24 hours on startup.
pub fn cleanup_stale_staging_sessions(app_data_dir: &Path) {
    let staging = app_data_dir.join("temp-images");
    info!("清理过期暂存文件 (目录: {})", staging.display());
    let entries = match std::fs::read_dir(&staging) {
        Ok(e) => e,
        Err(_) => return,
    };

    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(24 * 3600))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(metadata) = path.metadata() {
            if let Ok(modified) = metadata.modified() {
                if modified < cutoff {
                    if let Err(e) = std::fs::remove_dir_all(&path) {
                        warn!("警告: 无法清理过期暂存目录 {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
}

// ===== Save with Staged Images =====

/// Copy staged images to the permanent images directory.
/// Returns (relative_path, dest_absolute_path, original_name) for each image.
/// Does NOT delete staging sources — the caller cleans the session on success.
fn copy_staged_to_permanent(
    db: &DbState,
    session_id: &str,
    temp_ids: &[String],
) -> Result<Vec<(String, PathBuf, String)>, String> {
    let manifest = load_session_manifest(db, session_id);
    let mut saved = Vec::new();

    std::fs::create_dir_all(&db.images_dir).ok();

    for temp_id in temp_ids {
        let src = resolve_staged_path(db, session_id, temp_id)?;
        if !src.exists() {
            // Clean up any files we already copied
            for (_, p, _) in &saved {
                std::fs::remove_file(p).ok();
            }
            return Err(format!("图片 {} 已失效，请重新添加", temp_id));
        }

        let original_name = manifest
            .get(temp_id)
            .cloned()
            .unwrap_or_else(|| String::from("unknown"));

        let ext = Path::new(temp_id)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg");
        let new_name = format!("{}.{}", Uuid::new_v4(), ext);
        let dest = db.images_dir.join(&new_name);

        std::fs::copy(&src, &dest).map_err(|_| "暂存图片保存失败，请重试".to_string())?;

        let relative_path = format!("images/{}", new_name);
        saved.push((relative_path, dest, original_name));
    }

    Ok(saved)
}

fn clean_staging_session(db: &DbState, session_id: &str) {
    if let Ok(dir) = resolve_session_dir(db, session_id) {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).ok();
        }
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_record_with_staged_images(
    db: State<'_, DbState>,
    token: String,
    book_id: i64,
    date: String,
    service_content: String,
    specification: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    total_amount: i64,
    remark: Option<String>,
    session_id: String,
    temp_image_ids: Vec<String>,
) -> Result<IncomeRecord, String> {
    db.validate_token(&token).await?;
    crate::commands::record::validate_record_fields(
        &date,
        &service_content,
        quantity,
        unit_price,
        total_amount,
    )?;

    let specification = specification.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Copy staged files to permanent location (staged sources are preserved for retry)
    let copied = copy_staged_to_permanent(&db, &session_id, &temp_image_ids)?;

    let pool = db.get_pool().await?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let record_id = sqlx::query(
        r#"INSERT INTO income_records
        (book_id, date, service_content, specification, quantity, unit, unit_price, total_amount, remark, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind(book_id)
    .bind(&date)
    .bind(&service_content)
    .bind(&specification)
    .bind(quantity)
    .bind(&unit)
    .bind(unit_price)
    .bind(total_amount)
    .bind(&remark)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        // DB insert failed: clean up copied permanent files, keep staged
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        e.to_string()
    })?
    .last_insert_rowid();

    info!(
        "创建记录 id={} book={} amount={} date={} images={}",
        record_id,
        book_id,
        total_amount,
        date,
        copied.len()
    );

    let mut saved_images: Vec<IncomeImage> = Vec::new();
    for (relative_path, _dest_path, original_name) in &copied {
        let img_id = sqlx::query(
            "INSERT INTO income_images (record_id, file_path, original_name, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(record_id)
        .bind(relative_path)
        .bind(original_name)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            for (_, p, _) in &copied {
                std::fs::remove_file(p).ok();
            }
            e.to_string()
        })?
        .last_insert_rowid();

        saved_images.push(IncomeImage {
            id: img_id,
            record_id,
            file_path: relative_path.clone(),
            original_name: original_name.clone(),
            created_at: now.clone(),
        });
    }

    tx.commit().await.map_err(|e| {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        e.to_string()
    })?;

    // Only clean staging after successful commit — user can retry on failure
    clean_staging_session(&db, &session_id);

    Ok(IncomeRecord {
        id: record_id,
        book_id,
        date,
        service_content,
        specification,
        quantity,
        unit,
        unit_price,
        total_amount,
        settlement_status: "unsettled".into(),
        payment_date: None,
        payment_method: None,
        remark,
        images: saved_images,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_record_with_staged_images(
    db: State<'_, DbState>,
    token: String,
    id: i64,
    date: String,
    service_content: String,
    specification: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    total_amount: i64,
    remark: Option<String>,
    keep_image_ids: Vec<i64>,
    session_id: String,
    temp_image_ids: Vec<String>,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    crate::commands::record::validate_record_fields(
        &date,
        &service_content,
        quantity,
        unit_price,
        total_amount,
    )?;

    let specification = specification.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Copy staged files to permanent location (staged sources are preserved for retry)
    let copied = if !temp_image_ids.is_empty() {
        copy_staged_to_permanent(&db, &session_id, &temp_image_ids)?
    } else {
        Vec::new()
    };

    let pool = db.get_pool().await?;
    let mut tx = pool.begin().await.map_err(|e| {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        e.to_string()
    })?;

    // 在事务内检查结算状态，消除 TOCTOU 窗口
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                for (_, p, _) in &copied {
                    std::fs::remove_file(p).ok();
                }
                e.to_string()
            })?;
    let (current_status,) = status.ok_or_else(|| {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        "记录不存在".to_string()
    })?;
    if current_status == "settled" {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Err("已结清记录不可修改".into());
    }

    // Update record
    sqlx::query(
        r#"UPDATE income_records SET
        date = ?1, service_content = ?2, specification = ?3, quantity = ?4, unit = ?5, unit_price = ?6,
        total_amount = ?7, remark = ?8, updated_at = ?9
        WHERE id = ?10"#,
    )
    .bind(&date)
    .bind(&service_content)
    .bind(&specification)
    .bind(quantity)
    .bind(&unit)
    .bind(unit_price)
    .bind(total_amount)
    .bind(&remark)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        e.to_string()
    })?;

    // Delete images not in keep_image_ids (DB only, collect paths for later file cleanup)
    let removed_paths: Vec<String> = if !keep_image_ids.is_empty() {
        let placeholders: Vec<String> = keep_image_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        let del_query = format!(
            "DELETE FROM income_images WHERE record_id = ?1 AND id NOT IN ({}) RETURNING file_path",
            placeholders.join(", ")
        );
        let mut del_q = sqlx::query_scalar::<_, String>(&del_query).bind(id);
        for kid in &keep_image_ids {
            del_q = del_q.bind(kid);
        }
        del_q
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| {
                for (_, p, _) in &copied {
                    std::fs::remove_file(p).ok();
                }
                e.to_string()
            })?
    } else {
        let removed: Vec<(String,)> =
            sqlx::query_as("DELETE FROM income_images WHERE record_id = ?1 RETURNING file_path")
                .bind(id)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    for (_, p, _) in &copied {
                        std::fs::remove_file(p).ok();
                    }
                    e.to_string()
                })?;
        removed.into_iter().map(|(p,)| p).collect()
    };

    // Insert new images
    for (relative_path, _dest_path, original_name) in &copied {
        sqlx::query(
            "INSERT INTO income_images (record_id, file_path, original_name, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(id)
        .bind(relative_path)
        .bind(original_name)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            for (_, p, _) in &copied {
                std::fs::remove_file(p).ok();
            }
            e.to_string()
        })?;
    }

    tx.commit().await.map_err(|e| {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        e.to_string()
    })?;

    // Delete removed old image files AFTER successful commit
    for path in &removed_paths {
        if let Ok(full_path) = resolve_image_path(&db, path) {
            std::fs::remove_file(full_path).ok();
        }
    }

    // Only clean staging after successful commit — user can retry on failure
    clean_staging_session(&db, &session_id);

    Ok(())
}
