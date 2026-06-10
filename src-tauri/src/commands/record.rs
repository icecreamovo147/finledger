use crate::db::DbState;
use crate::models::{IncomeImage, IncomeRecord, PaginatedRecords};
use crate::utils::escape_like;
use base64::Engine;
use sqlx::FromRow;
use std::path::{Path, PathBuf};
use tauri::State;
use tracing::{info, warn};

/// Intermediate row struct for income_records queries (replaces 15-element tuple).
#[derive(FromRow)]
struct IncomeRecordRow {
    id: i64,
    book_id: i64,
    date: String,
    service_content: String,
    specification: String,
    quantity: Option<i64>,
    unit: String,
    unit_price: Option<i64>,
    total_amount: i64,
    settlement_status: String,
    payment_date: Option<String>,
    payment_method: Option<String>,
    remark: String,
    created_at: String,
    updated_at: String,
}

impl IncomeRecordRow {
    fn into_record(self, images: Vec<IncomeImage>) -> IncomeRecord {
        IncomeRecord {
            id: self.id,
            book_id: self.book_id,
            date: self.date,
            service_content: self.service_content,
            specification: self.specification,
            quantity: self.quantity,
            unit: self.unit,
            unit_price: self.unit_price,
            total_amount: self.total_amount,
            settlement_status: self.settlement_status,
            payment_date: self.payment_date,
            payment_method: self.payment_method,
            remark: self.remark,
            images,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Intermediate row struct for income_images queries (replaces 5-element tuple).
#[derive(FromRow)]
struct IncomeImageRow {
    id: i64,
    record_id: i64,
    file_path: String,
    original_name: String,
    created_at: String,
}

impl From<IncomeImageRow> for IncomeImage {
    fn from(row: IncomeImageRow) -> Self {
        IncomeImage {
            id: row.id,
            record_id: row.record_id,
            file_path: row.file_path,
            original_name: row.original_name,
            created_at: row.created_at,
        }
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

pub fn validate_record_fields(
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

pub fn bytes_to_data_url(bytes: &[u8], original_name: &str) -> String {
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
    let data_q = bind_filters!(sqlx::query_as::<_, IncomeRecordRow>(&data_query));
    let rows = data_q
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let record_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();

    let all_images: Vec<IncomeImageRow> = if record_ids.is_empty() {
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
        let mut img_q = sqlx::query_as::<_, IncomeImageRow>(&img_query);
        for rid in &record_ids {
            img_q = img_q.bind(rid);
        }
        img_q.fetch_all(&pool).await.map_err(|e| e.to_string())?
    };

    let mut images_by_record: std::collections::HashMap<i64, Vec<IncomeImage>> =
        std::collections::HashMap::new();
    for img_row in all_images {
        images_by_record
            .entry(img_row.record_id)
            .or_default()
            .push(img_row.into());
    }

    let records: Vec<IncomeRecord> = rows
        .into_iter()
        .map(|row| {
            let id = row.id;
            row.into_record(images_by_record.remove(&id).unwrap_or_default())
        })
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

    info!(
        "创建记录 id={} book={} amount={} date={}",
        id, book_id, total_amount, date
    );

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

    let row = sqlx::query_as::<_, IncomeRecordRow>(
        "SELECT id, book_id, date, service_content, specification, quantity, unit, unit_price, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let row = row.ok_or("记录不存在")?;

    let images: Vec<IncomeImageRow> = sqlx::query_as(
        "SELECT id, record_id, file_path, original_name, created_at FROM income_images WHERE record_id = ?1",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.into_record(images.into_iter().map(Into::into).collect()))
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

    // 使用事务包裹 SELECT + DELETE，消除 TOCTOU 窗口
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT file_path, record_id FROM income_images WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    let (path, record_id) = row.ok_or("图片不存在")?;

    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(record_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Err("已结清记录不可修改".into());
    }

    // 在事务内删除 DB 行
    sqlx::query("DELETE FROM income_images WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 事务提交后再解析路径和删除物理文件
    let full_path = resolve_image_path(&db, &path)?;
    tx.commit().await.map_err(|e| e.to_string())?;

    // After commit, delete the file (best-effort)
    if let Err(e) = std::fs::remove_file(&full_path) {
        warn!("警告: 无法删除图片文件 {}: {}", full_path.display(), e);
    }

    Ok(())
}

// ===== Settlement =====

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
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        tx.rollback().await.map_err(|e| e.to_string())?;
        warn!("结清操作冲突: 记录 {} 状态已变更", id);
        return Err("该记录状态已变更，请刷新后重试".into());
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    info!(
        "记录 {} 已标记为结清 payment_date={} method={}",
        id, payment_date, payment_method
    );

    Ok(())
}

#[tauri::command]
pub async fn unsettle_record(db: State<'_, DbState>, token: String, id: i64) -> Result<(), String> {
    db.validate_token(&token).await?;
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
    if current_status != "settled" {
        tx.rollback().await.map_err(|e| e.to_string())?;
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
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        tx.rollback().await.map_err(|e| e.to_string())?;
        warn!("回退操作冲突: 记录 {} 状态已变更", id);
        return Err("该记录状态已变更，请刷新后重试".into());
    }
    tx.commit().await.map_err(|e| e.to_string())?;
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
