use crate::db::DbState;
use crate::models::{IncomeImage, IncomeRecord, PaginatedRecords};
use crate::utils::escape_like;
use base64::Engine;
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
        // Return the original extension to preserve case from the source
        Ok(ext)
    } else if ext.is_empty() {
        Err("无法识别图片格式，请选择常见图片文件".into())
    } else {
        Err(format!("不支持的图片格式 .{}，支持：jpg, jpeg, png, gif, bmp, webp", ext))
    }
}

/// Helper to build dynamic WHERE clauses with consistent bind-parameter indexing.
struct FilterBuilder {
    conditions: Vec<String>,
    next_index: u32,
}

impl FilterBuilder {
    fn new(base_condition: &str) -> Self {
        Self {
            conditions: vec![base_condition.to_string()],
            next_index: 2,
        }
    }

    /// Add a single-parameter condition. Returns the assigned bind index.
    fn add_condition(&mut self, clause: &str) -> u32 {
        let idx = self.next_index;
        self.conditions
            .push(clause.replace("?", &format!("?{}", idx)));
        self.next_index += 1;
        idx
    }

    fn where_clause(&self) -> String {
        self.conditions.join(" AND ")
    }

    fn current_index(&self) -> u32 {
        self.next_index
    }
}

fn validate_record_fields(
    date: &str,
    service_content: &str,
    quantity: Option<i64>,
    unit_price: Option<i64>,
    total_amount: i64,
) -> Result<(), String> {
    if date.is_empty() {
        return Err("日期不能为空".into());
    }
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "日期格式无效，应为 YYYY-MM-DD".to_string())?;
    if service_content.trim().is_empty() {
        return Err("服务项目及内容不能为空".into());
    }
    if total_amount < 0 {
        return Err("金额不能为负数".into());
    }
    if let Some(qty) = quantity {
        if qty < 0 {
            return Err("数量不能为负数".into());
        }
    }
    if let Some(price) = unit_price {
        if price < 0 {
            return Err("单价不能为负数".into());
        }
    }
    if let (Some(qty), Some(price)) = (quantity, unit_price) {
        let expected = qty
            .checked_mul(price)
            .ok_or_else(|| "数量×单价超出允许范围".to_string())?;
        if total_amount != expected {
            return Err(format!(
                "总金额与数量×单价不一致：{} × {} = {}，但传入 {}",
                qty, price, expected, total_amount
            ));
        }
    }
    Ok(())
}

pub fn resolve_image_path(db: &DbState, stored_path: &str) -> Result<PathBuf, String> {
    let normalized = stored_path.replace('\\', "/");
    let path = Path::new(&normalized);

    // Reject absolute paths — all image paths must be relative
    if path.is_absolute() {
        return Err("非法的图片路径".into());
    }

    // Reject paths that try to escape via parent directory traversal
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err("非法的图片路径".into());
        }
    }

    let resolved = if normalized.starts_with("images/") {
        db.app_data_dir.join(path)
    } else {
        db.images_dir.join(path)
    };

    // If file exists, canonicalize and verify it's within images_dir.
    // Canonicalize images_dir as well to handle symlinks (e.g. macOS /var → /private/var).
    if let Ok(canonical) = resolved.canonicalize() {
        let canonical_images_dir = db
            .images_dir
            .canonicalize()
            .unwrap_or_else(|_| db.images_dir.clone());
        if canonical.starts_with(&canonical_images_dir) {
            return Ok(canonical);
        }
        return Err("非法的图片路径".into());
    }

    // File doesn't exist — component check already prevents .. traversal
    Ok(resolved)
}

fn bytes_to_data_url(bytes: &[u8], original_name: &str) -> String {
    let mime = mime_guess::from_path(original_name)
        .first_or_octet_stream()
        .to_string();
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{};base64,{}", mime, b64)
}

pub async fn do_list_records(
    db: &DbState,
    book_id: i64,
    settlement_status: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedRecords, String> {
    let pool = db.get_pool().await?;

    let mut fb = FilterBuilder::new("book_id = ?1");
    if settlement_status.is_some() {
        fb.add_condition("settlement_status = ?");
    }
    if date_from.is_some() {
        fb.add_condition("date >= ?");
    }
    if date_to.is_some() {
        fb.add_condition("date <= ?");
    }
    let keyword_like: Option<String> = if let Some(ref kw) = keyword {
        let like = format!("%{}%", escape_like(&kw.to_lowercase()));
        let i = fb.next_index;
        fb.conditions.push(format!(
            "(LOWER(service_content) LIKE ?{} ESCAPE '\\' OR LOWER(remark) LIKE ?{} ESCAPE '\\')",
            i,
            i + 1
        ));
        fb.next_index += 2;
        Some(like)
    } else {
        None
    };

    let where_clause = fb.where_clause();

    macro_rules! bind_filters {
        ($q:expr) => {{
            let mut q = $q.bind(book_id);
            if let Some(ref status) = settlement_status {
                q = q.bind(status);
            }
            if let Some(ref from) = date_from {
                q = q.bind(from);
            }
            if let Some(ref to) = date_to {
                q = q.bind(to);
            }
            if let Some(ref like) = keyword_like {
                q = q.bind(like.clone());
                q = q.bind(like.clone());
            }
            q
        }};
    }

    let count_query = format!("SELECT COUNT(*) FROM income_records WHERE {}", where_clause);
    let total = bind_filters!(sqlx::query_as::<_, (i64,)>(&count_query))
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?
        .0;

    let unsettled_query = format!(
        "SELECT COALESCE(SUM(total_amount), 0) FROM income_records WHERE {} AND settlement_status = 'unsettled'",
        where_clause
    );
    let total_unsettled = bind_filters!(sqlx::query_as::<_, (i64,)>(&unsettled_query))
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?
        .0;

    let book_total: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(total_amount), 0) FROM income_records WHERE book_id = ?1 AND settlement_status = 'unsettled'",
    )
    .bind(book_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let limit = page_size.unwrap_or(20);
    let offset = ((page.unwrap_or(1) - 1).max(0)) * limit;
    let limit_idx = fb.current_index();
    let offset_idx = limit_idx + 1;
    let data_query = format!(
        "SELECT id, book_id, date, service_content, specification, quantity, unit, unit_price, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE {} ORDER BY date DESC, id DESC LIMIT ?{} OFFSET ?{}",
        where_clause, limit_idx, offset_idx
    );
    let data_q = bind_filters!(sqlx::query_as::<
        _,
        (
            i64,
            i64,
            String,
            String,
            String,
            Option<i64>,
            String,
            Option<i64>,
            i64,
            String,
            Option<String>,
            Option<String>,
            String,
            String,
            String,
        ),
    >(&data_query));
    let rows = data_q
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let record_ids: Vec<i64> = rows.iter().map(|r| r.0).collect();

    let all_images: Vec<(i64, i64, String, String, String)> = if record_ids.is_empty() {
        vec![]
    } else {
        let placeholders: Vec<String> = record_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let img_query = format!(
            "SELECT id, record_id, file_path, original_name, created_at FROM income_images WHERE record_id IN ({}) ORDER BY id",
            placeholders.join(", ")
        );
        let mut img_q = sqlx::query_as::<_, (i64, i64, String, String, String)>(&img_query);
        for rid in &record_ids {
            img_q = img_q.bind(rid);
        }
        img_q.fetch_all(&pool).await.map_err(|e| e.to_string())?
    };

    let mut images_by_record: std::collections::HashMap<i64, Vec<IncomeImage>> =
        std::collections::HashMap::new();
    for (img_id, record_id, file_path, original_name, created_at) in all_images {
        images_by_record
            .entry(record_id)
            .or_default()
            .push(IncomeImage {
                id: img_id,
                record_id,
                file_path,
                original_name,
                created_at,
            });
    }

    let records: Vec<IncomeRecord> = rows
        .into_iter()
        .map(
            |(
                id,
                book_id,
                date,
                service_content,
                specification,
                quantity,
                unit,
                unit_price,
                total_amount,
                settlement_status,
                payment_date,
                payment_method,
                remark,
                created_at,
                updated_at,
            )| {
                IncomeRecord {
                    id,
                    book_id,
                    date,
                    service_content,
                    specification,
                    quantity,
                    unit,
                    unit_price,
                    total_amount,
                    settlement_status,
                    payment_date,
                    payment_method,
                    remark,
                    images: images_by_record.remove(&id).unwrap_or_default(),
                    created_at,
                    updated_at,
                }
            },
        )
        .collect();

    Ok(PaginatedRecords {
        total,
        total_unsettled,
        book_total_unsettled: book_total.0,
        records,
    })
}

pub async fn do_update_record(
    db: &DbState,
    id: i64,
    date: String,
    service_content: String,
    specification: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    total_amount: i64,
    remark: Option<String>,
) -> Result<(), String> {
    let pool = db.get_pool().await?;
    // 使用事务包裹 SELECT + UPDATE，消除 TOCTOU 窗口
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Err("已结清记录不可修改".into());
    }
    validate_record_fields(&date, &service_content, quantity, unit_price, total_amount)?;
    let specification = specification.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        r#"UPDATE income_records SET
        date = ?1, service_content = ?2, specification = ?3, quantity = ?4, unit = ?5, unit_price = ?6,
        total_amount = ?7, remark = ?8, updated_at = ?9
        WHERE id = ?10 AND settlement_status = 'unsettled'"#,
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
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn do_delete_record(db: &DbState, id: i64) -> Result<(), String> {
    let pool = db.get_pool().await?;

    // 在事务中完成所有数据库操作，消除 TOCTOU 窗口
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 在事务内检查结算状态
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Err("已结清记录不可删除".into());
    }

    // 在事务内查询图片路径
    let images: Vec<(String,)> =
        sqlx::query_as("SELECT file_path FROM income_images WHERE record_id = ?1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    // 删除图片记录和收入记录（在同一个事务中）
    sqlx::query("DELETE FROM income_images WHERE record_id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM income_records WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;

    // After commit, delete image files (best-effort, don't fail on error)
    for (path,) in &images {
        if let Ok(full_path) = resolve_image_path(db, path) {
            if let Err(e) = std::fs::remove_file(&full_path) {
                warn!("警告: 无法删除图片文件 {}: {}", full_path.display(), e);
            }
        }
    }
    Ok(())
}

// ===== Tauri command wrappers =====

#[tauri::command]
pub async fn create_record(
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
) -> Result<IncomeRecord, String> {
    db.validate_token(&token).await?;
    validate_record_fields(&date, &service_content, quantity, unit_price, total_amount)?;

    let specification = specification.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let pool = db.get_pool().await?;
    let id = sqlx::query(
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
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();

    info!("创建记录 id={} book={} amount={} date={}", id, book_id, total_amount, date);

    Ok(IncomeRecord {
        id,
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
        images: vec![],
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn list_records(
    db: State<'_, DbState>,
    token: String,
    book_id: i64,
    settlement_status: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedRecords, String> {
    db.validate_token(&token).await?;
    do_list_records(
        &db,
        book_id,
        settlement_status,
        date_from,
        date_to,
        keyword,
        page,
        page_size,
    )
    .await
}

#[tauri::command]
pub async fn get_record(
    db: State<'_, DbState>,
    token: String,
    id: i64,
) -> Result<IncomeRecord, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    let row: Option<(
        i64, i64, String, String, String, Option<i64>, String, Option<i64>,
        i64, String, Option<String>, Option<String>, String, String, String,
    )> = sqlx::query_as(
        "SELECT id, book_id, date, service_content, specification, quantity, unit, unit_price, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (
        id,
        book_id,
        date,
        service_content,
        specification,
        quantity,
        unit,
        unit_price,
        total_amount,
        settlement_status,
        payment_date,
        payment_method,
        remark,
        created_at,
        updated_at,
    ) = row.ok_or("记录不存在")?;

    let images: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, record_id, file_path, original_name, created_at FROM income_images WHERE record_id = ?1",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(IncomeRecord {
        id,
        book_id,
        date,
        service_content,
        specification,
        quantity,
        unit,
        unit_price,
        total_amount,
        settlement_status,
        payment_date,
        payment_method,
        remark,
        images: images
            .into_iter()
            .map(
                |(img_id, record_id, file_path, original_name, created_at)| IncomeImage {
                    id: img_id,
                    record_id,
                    file_path,
                    original_name,
                    created_at,
                },
            )
            .collect(),
        created_at,
        updated_at,
    })
}

#[tauri::command]
pub async fn update_record(
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
) -> Result<(), String> {
    db.validate_token(&token).await?;
    do_update_record(
        &db,
        id,
        date,
        service_content,
        specification,
        quantity,
        unit,
        unit_price,
        total_amount,
        remark,
    )
    .await
}

#[tauri::command]
pub async fn delete_record(db: State<'_, DbState>, token: String, id: i64) -> Result<(), String> {
    db.validate_token(&token).await?;
    do_delete_record(&db, id).await
}

#[tauri::command]
pub async fn delete_image(db: State<'_, DbState>, token: String, id: i64) -> Result<(), String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT file_path, record_id FROM income_images WHERE id = ?1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (path, record_id) = row.ok_or("图片不存在")?;

    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(record_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        return Err("已结清记录不可修改".into());
    }

    // Delete DB row first in a transaction
    let full_path = resolve_image_path(&db, &path)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM income_images WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;

    // After commit, delete the file (best-effort)
    if let Err(e) = std::fs::remove_file(&full_path) {
        warn!("警告: 无法删除图片文件 {}: {}", full_path.display(), e);
    }

    Ok(())
}

// ===== T14: Settlement =====

#[tauri::command]
pub async fn settle_record(
    db: State<'_, DbState>,
    token: String,
    id: i64,
    payment_date: String,
    payment_method: String,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    if payment_date.is_empty() {
        return Err("收款日期不能为空".into());
    }
    if payment_method.is_empty() {
        return Err("收款方式不能为空".into());
    }

    let pool = db.get_pool().await?;
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        return Err("该记录已是已结清状态".into());
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let result = sqlx::query(
        r#"UPDATE income_records SET
        settlement_status = 'settled', payment_date = ?1, payment_method = ?2, updated_at = ?3
        WHERE id = ?4 AND settlement_status = 'unsettled'"#,
    )
    .bind(&payment_date)
    .bind(&payment_method)
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        warn!("结清操作冲突: 记录 {} 状态已变更", id);
        return Err("该记录状态已变更，请刷新后重试".into());
    }
    info!("记录 {} 已标记为结清 payment_date={} method={}", id, payment_date, payment_method);

    Ok(())
}

#[tauri::command]
pub async fn unsettle_record(db: State<'_, DbState>, token: String, id: i64) -> Result<(), String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status != "settled" {
        return Err("只有已结清记录才能回退".into());
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let result = sqlx::query(
        r#"UPDATE income_records SET
        settlement_status = 'unsettled', payment_date = NULL, payment_method = NULL, updated_at = ?1
        WHERE id = ?2 AND settlement_status = 'settled'"#,
    )
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        warn!("回退操作冲突: 记录 {} 状态已变更", id);
        return Err("该记录状态已变更，请刷新后重试".into());
    }
    info!("记录 {} 已回退为未结清", id);

    Ok(())
}

#[tauri::command]
pub async fn read_image_base64(
    db: State<'_, DbState>,
    token: String,
    image_id: i64,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT file_path, original_name FROM income_images WHERE id = ?1")
            .bind(image_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (relative_path, original_name) = row.ok_or("图片不存在")?;
    let full_path = resolve_image_path(&db, &relative_path)?;

    let bytes = std::fs::read(&full_path).map_err(|e| format!("读取图片失败: {}", e))?;

    Ok(bytes_to_data_url(&bytes, &original_name))
}


// ===== Staged Image Upload =====

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
            // 尝试直接写入作为兜底
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
    validate_record_fields(&date, &service_content, quantity, unit_price, total_amount)?;

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

    info!("创建记录 id={} book={} amount={} date={} images={}", record_id, book_id, total_amount, date, copied.len());

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
    validate_record_fields(&date, &service_content, quantity, unit_price, total_amount)?;

    let pool = db.get_pool().await?;

    // Check not settled
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;
    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        return Err("已结清记录不可修改".into());
    }

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

    let mut tx = pool.begin().await.map_err(|e| {
        for (_, p, _) in &copied {
            std::fs::remove_file(p).ok();
        }
        e.to_string()
    })?;

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

// ===== Attachment Consistency Check =====

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OrphanImage {
    pub id: i64,
    pub record_id: i64,
    pub file_path: String,
}

pub async fn do_check_image_consistency(db: &DbState) -> Result<Vec<OrphanImage>, String> {
    let pool = db.get_pool().await?;
    let rows: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT id, record_id, file_path FROM income_images")
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let mut orphans = Vec::new();
    for (id, record_id, file_path) in rows {
        let is_orphan = match resolve_image_path(db, &file_path) {
            Ok(full_path) => !full_path.exists(),
            Err(_) => true, // invalid path → treat as orphan
        };
        if is_orphan {
            orphans.push(OrphanImage {
                id,
                record_id,
                file_path,
            });
        }
    }
    Ok(orphans)
}

#[tauri::command]
pub async fn check_attachment_consistency(
    db: State<'_, DbState>,
    token: String,
) -> Result<Vec<OrphanImage>, String> {
    db.validate_token(&token).await?;
    do_check_image_consistency(&db).await
}

#[tauri::command]
pub async fn cleanup_orphan_images(db: State<'_, DbState>, token: String) -> Result<u64, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 在事务内查询并删除孤儿图片，消除 TOCTOU 窗口
    let rows: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT id, record_id, file_path FROM income_images")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    let mut orphan_ids = Vec::new();
    for (id, _record_id, file_path) in &rows {
        let is_orphan = match resolve_image_path(&db, file_path) {
            Ok(full_path) => !full_path.exists(),
            Err(_) => true,
        };
        if is_orphan {
            orphan_ids.push(*id);
        }
    }

    let count = orphan_ids.len() as u64;
    for id in &orphan_ids {
        sqlx::query("DELETE FROM income_images WHERE id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(count)
}
