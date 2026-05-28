use crate::db::DbState;
use crate::models::{ImageUpload, IncomeImage, IncomeRecord, PaginatedRecords};
use base64::Engine;
use std::path::{Path, PathBuf};
use tauri::State;
use uuid::Uuid;

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
        self.conditions.push(clause.replace("?", &format!("?{}", idx)));
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
    category: &str,
    quantity: Option<i64>,
    unit_price: Option<i64>,
    total_amount: i64,
) -> Result<(), String> {
    if date.is_empty() {
        return Err("日期不能为空".into());
    }
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "日期格式无效，应为 YYYY-MM-DD".to_string())?;
    if category.is_empty() {
        return Err("类别不能为空".into());
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
        let expected = qty * price;
        if total_amount != expected {
            return Err(format!(
                "总金额与数量×单价不一致：{} × {} = {}，但传入 {}",
                qty, price, expected, total_amount
            ));
        }
    }
    Ok(())
}

pub fn resolve_image_path(db: &DbState, stored_path: &str) -> PathBuf {
    let normalized = stored_path.replace('\\', "/");
    let path = Path::new(&normalized);

    if path.is_absolute() {
        path.to_path_buf()
    } else if normalized.starts_with("images/") {
        db.app_data_dir.join(path)
    } else {
        db.images_dir.join(path)
    }
}

// ===== Internal helpers (take &DbState, testable without Tauri) =====

pub async fn do_list_records(
    db: &DbState,
    book_id: i64,
    category: Option<String>,
    settlement_status: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedRecords, String> {
    let pool = db.get_pool().await?;

    let mut fb = FilterBuilder::new("book_id = ?1");
    if category.is_some() {
        fb.add_condition("category = ?");
    }
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
        let like = format!("%{}%", kw.to_lowercase());
        let i = fb.next_index;
        fb.conditions
            .push(format!("(LOWER(description) LIKE ?{} OR LOWER(remark) LIKE ?{})", i, i + 1));
        fb.next_index += 2;
        Some(like)
    } else {
        None
    };

    let where_clause = fb.where_clause();

    macro_rules! bind_filters {
        ($q:expr) => {{
            let mut q = $q.bind(book_id);
            if let Some(ref cat) = category { q = q.bind(cat); }
            if let Some(ref status) = settlement_status { q = q.bind(status); }
            if let Some(ref from) = date_from { q = q.bind(from); }
            if let Some(ref to) = date_to { q = q.bind(to); }
            if let Some(ref like) = keyword_like {
                q = q.bind(like.clone());
                q = q.bind(like.clone());
            }
            q
        }};
    }

    let count_query = format!("SELECT COUNT(*) FROM income_records WHERE {}", where_clause);
    let total = bind_filters!(sqlx::query_as::<_, (i64,)>(&count_query))
        .fetch_one(&pool).await.map_err(|e| e.to_string())?.0;

    let unsettled_query = format!(
        "SELECT COALESCE(SUM(total_amount), 0) FROM income_records WHERE {} AND settlement_status = 'unsettled'",
        where_clause
    );
    let total_unsettled = bind_filters!(sqlx::query_as::<_, (i64,)>(&unsettled_query))
        .fetch_one(&pool).await.map_err(|e| e.to_string())?.0;

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
        "SELECT id, book_id, date, category, description, quantity, unit, unit_price, size_info, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE {} ORDER BY date DESC, id DESC LIMIT ?{} OFFSET ?{}",
        where_clause, limit_idx, offset_idx
    );
    let data_q = bind_filters!(sqlx::query_as::<_, (
        i64, i64, String, String, String, Option<i64>, String, Option<i64>, String,
        i64, String, Option<String>, Option<String>, String, String, String,
    )>(&data_query));
    let rows = data_q.bind(limit).bind(offset)
        .fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let record_ids: Vec<i64> = rows.iter().map(|r| r.0).collect();

    let all_images: Vec<(i64, i64, String, String, String)> = if record_ids.is_empty() {
        vec![]
    } else {
        let placeholders: Vec<String> = record_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
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

    let mut images_by_record: std::collections::HashMap<i64, Vec<IncomeImage>> = std::collections::HashMap::new();
    for (img_id, record_id, file_path, original_name, created_at) in all_images {
        images_by_record.entry(record_id).or_default().push(IncomeImage {
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
                id, book_id, date, category, description, quantity, unit, unit_price, size_info,
                total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at,
            )| {
                IncomeRecord {
                    id, book_id, date, category, description, quantity, unit, unit_price, size_info,
                    total_amount, settlement_status, payment_date, payment_method, remark,
                    images: images_by_record.remove(&id).unwrap_or_default(),
                    created_at, updated_at,
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
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    size_info: Option<String>,
    total_amount: i64,
    remark: Option<String>,
) -> Result<(), String> {
    let pool = db.get_pool().await?;
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
    validate_record_fields(&date, &category, quantity, unit_price, total_amount)?;
    let description = description.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let size_info = size_info.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        r#"UPDATE income_records SET
        date = ?1, category = ?2, description = ?3, quantity = ?4, unit = ?5, unit_price = ?6,
        size_info = ?7, total_amount = ?8, remark = ?9, updated_at = ?10
        WHERE id = ?11"#,
    )
    .bind(&date)
    .bind(&category)
    .bind(&description)
    .bind(quantity)
    .bind(&unit)
    .bind(unit_price)
    .bind(&size_info)
    .bind(total_amount)
    .bind(&remark)
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn do_delete_record(db: &DbState, id: i64) -> Result<(), String> {
    let pool = db.get_pool().await?;
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;
    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        return Err("已结清记录不可删除".into());
    }

    // Query image paths before deletion
    let images: Vec<(String,)> =
        sqlx::query_as("SELECT file_path FROM income_images WHERE record_id = ?1")
            .bind(id)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;

    // Delete DB rows in a transaction first
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
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
        let full_path = resolve_image_path(db, path);
        if let Err(e) = std::fs::remove_file(&full_path) {
            eprintln!("警告: 无法删除图片文件 {}: {}", full_path.display(), e);
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
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    size_info: Option<String>,
    total_amount: i64,
    remark: Option<String>,
) -> Result<IncomeRecord, String> {
    db.validate_token(&token).await?;
    validate_record_fields(&date, &category, quantity, unit_price, total_amount)?;

    let description = description.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let size_info = size_info.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let pool = db.get_pool().await?;
    let id = sqlx::query(
        r#"INSERT INTO income_records
        (book_id, date, category, description, quantity, unit, unit_price, size_info, total_amount, remark, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
    )
    .bind(book_id)
    .bind(&date)
    .bind(&category)
    .bind(&description)
    .bind(quantity)
    .bind(&unit)
    .bind(unit_price)
    .bind(&size_info)
    .bind(total_amount)
    .bind(&remark)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();

    Ok(IncomeRecord {
        id,
        book_id,
        date,
        category,
        description,
        quantity,
        unit,
        unit_price,
        size_info,
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
    category: Option<String>,
    settlement_status: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedRecords, String> {
    db.validate_token(&token).await?;
    do_list_records(&db, book_id, category, settlement_status, date_from, date_to, keyword, page, page_size).await
}

#[tauri::command]
pub async fn get_record(db: State<'_, DbState>, token: String, id: i64) -> Result<IncomeRecord, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    let row: Option<(
        i64, i64, String, String, String, Option<i64>, String, Option<i64>, String,
        i64, String, Option<String>, Option<String>, String, String, String,
    )> = sqlx::query_as(
        "SELECT id, book_id, date, category, description, quantity, unit, unit_price, size_info, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (
        id, book_id, date, category, description, quantity, unit, unit_price, size_info,
        total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at,
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
        category,
        description,
        quantity,
        unit,
        unit_price,
        size_info,
        total_amount,
        settlement_status,
        payment_date,
        payment_method,
        remark,
        images: images
            .into_iter()
            .map(|(img_id, record_id, file_path, original_name, created_at)| IncomeImage {
                id: img_id,
                record_id,
                file_path,
                original_name,
                created_at,
            })
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
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    size_info: Option<String>,
    total_amount: i64,
    remark: Option<String>,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    do_update_record(&db, id, date, category, description, quantity, unit, unit_price, size_info, total_amount, remark).await
}

#[tauri::command]
pub async fn delete_record(db: State<'_, DbState>, token: String, id: i64) -> Result<(), String> {
    db.validate_token(&token).await?;
    do_delete_record(&db, id).await
}

// ===== T11: Image Upload/Delete =====

#[tauri::command]
pub async fn upload_image(
    db: State<'_, DbState>,
    token: String,
    record_id: i64,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<IncomeImage, String> {
    db.validate_token(&token).await?;
    if file_bytes.len() > 20 * 1024 * 1024 {
        return Err("图片大小不能超过 20MB".into());
    }

    let pool = db.get_pool().await?;

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

    let ext = Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let new_name = format!("{}.{}", Uuid::new_v4(), ext);
    let temp_name = format!(".tmp-{}", new_name);

    std::fs::create_dir_all(&db.images_dir).ok();
    let temp_path = db.images_dir.join(&temp_name);
    let final_path = db.images_dir.join(&new_name);

    // Write to temp file first
    std::fs::write(&temp_path, &file_bytes).map_err(|e| e.to_string())?;

    let relative_path = format!("images/{}", new_name);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let result = sqlx::query(
        "INSERT INTO income_images (record_id, file_path, original_name, created_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(record_id)
    .bind(&relative_path)
    .bind(&file_name)
    .bind(&now)
    .execute(&pool)
    .await;

    match result {
        Ok(res) => {
            // DB insert succeeded: rename temp to final
            if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                // Rename failed: roll back the DB insert and clean up
                let id = res.last_insert_rowid();
                sqlx::query("DELETE FROM income_images WHERE id = ?1")
                    .bind(id)
                    .execute(&pool)
                    .await
                    .ok();
                std::fs::remove_file(&temp_path).ok();
                return Err(format!("保存图片文件失败: {}", e));
            }
            let id = res.last_insert_rowid();
            Ok(IncomeImage {
                id,
                record_id,
                file_path: relative_path,
                original_name: file_name,
                created_at: now,
            })
        }
        Err(e) => {
            // DB insert failed: clean up temp file
            std::fs::remove_file(&temp_path).ok();
            Err(e.to_string())
        }
    }
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
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM income_images WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;

    // After commit, delete the file (best-effort)
    let full_path = resolve_image_path(&db, &path);
    if let Err(e) = std::fs::remove_file(&full_path) {
        eprintln!("警告: 无法删除图片文件 {}: {}", full_path.display(), e);
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

    sqlx::query(
        r#"UPDATE income_records SET
        settlement_status = 'settled', payment_date = ?1, payment_method = ?2, updated_at = ?3
        WHERE id = ?4"#,
    )
    .bind(&payment_date)
    .bind(&payment_method)
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

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

    sqlx::query(
        r#"UPDATE income_records SET
        settlement_status = 'unsettled', payment_date = NULL, payment_method = NULL, updated_at = ?1
        WHERE id = ?2"#,
    )
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ===== Atomic Record+Images =====

fn save_image_file(db: &DbState, file_bytes: &[u8], file_name: &str) -> Result<(String, PathBuf), String> {
    if file_bytes.len() > 20 * 1024 * 1024 {
        return Err("图片大小不能超过 20MB".into());
    }
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let new_name = format!("{}.{}", Uuid::new_v4(), ext);
    std::fs::create_dir_all(&db.images_dir).ok();
    let save_path = db.images_dir.join(&new_name);
    std::fs::write(&save_path, file_bytes).map_err(|e| e.to_string())?;
    Ok((format!("images/{}", new_name), save_path))
}

#[tauri::command]
pub async fn create_record_with_images(
    db: State<'_, DbState>,
    token: String,
    book_id: i64,
    date: String,
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    size_info: Option<String>,
    total_amount: i64,
    remark: Option<String>,
    images: Vec<ImageUpload>,
) -> Result<IncomeRecord, String> {
    db.validate_token(&token).await?;
    validate_record_fields(&date, &category, quantity, unit_price, total_amount)?;

    let description = description.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let size_info = size_info.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let pool = db.get_pool().await?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let record_id = sqlx::query(
        r#"INSERT INTO income_records
        (book_id, date, category, description, quantity, unit, unit_price, size_info, total_amount, remark, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
    )
    .bind(book_id)
    .bind(&date)
    .bind(&category)
    .bind(&description)
    .bind(quantity)
    .bind(&unit)
    .bind(unit_price)
    .bind(&size_info)
    .bind(total_amount)
    .bind(&remark)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();

    let mut saved_files: Vec<PathBuf> = Vec::new();
    let mut saved_images: Vec<IncomeImage> = Vec::new();

    let insert_result: Result<(), String> = async {
        for img in &images {
            let (relative_path, save_path) = save_image_file(&db, &img.file_bytes, &img.file_name)?;
            saved_files.push(save_path);

            let img_id = sqlx::query(
                "INSERT INTO income_images (record_id, file_path, original_name, created_at) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(record_id)
            .bind(&relative_path)
            .bind(&img.file_name)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .last_insert_rowid();

            saved_images.push(IncomeImage {
                id: img_id,
                record_id,
                file_path: relative_path,
                original_name: img.file_name.clone(),
                created_at: now.clone(),
            });
        }
        Ok(())
    }.await;

    if let Err(e) = insert_result {
        tx.rollback().await.ok();
        for f in &saved_files { std::fs::remove_file(f).ok(); }
        return Err(e);
    }

    tx.commit().await.map_err(|e| {
        for f in &saved_files { std::fs::remove_file(f).ok(); }
        e.to_string()
    })?;

    Ok(IncomeRecord {
        id: record_id,
        book_id,
        date,
        category,
        description,
        quantity,
        unit,
        unit_price,
        size_info,
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
pub async fn update_record_with_images(
    db: State<'_, DbState>,
    token: String,
    id: i64,
    date: String,
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<i64>,
    size_info: Option<String>,
    total_amount: i64,
    remark: Option<String>,
    keep_image_ids: Vec<i64>,
    new_images: Vec<ImageUpload>,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    validate_record_fields(&date, &category, quantity, unit_price, total_amount)?;

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

    let description = description.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let size_info = size_info.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // Update record
    sqlx::query(
        r#"UPDATE income_records SET
        date = ?1, category = ?2, description = ?3, quantity = ?4, unit = ?5, unit_price = ?6,
        size_info = ?7, total_amount = ?8, remark = ?9, updated_at = ?10
        WHERE id = ?11"#,
    )
    .bind(&date)
    .bind(&category)
    .bind(&description)
    .bind(quantity)
    .bind(&unit)
    .bind(unit_price)
    .bind(&size_info)
    .bind(total_amount)
    .bind(&remark)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // Delete images not in keep_image_ids (DB only, collect paths for later file cleanup)
    let removed_paths: Vec<String> = if !keep_image_ids.is_empty() {
        let placeholders: Vec<String> = keep_image_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
        let del_query = format!(
            "DELETE FROM income_images WHERE record_id = ?1 AND id NOT IN ({}) RETURNING file_path",
            placeholders.join(", ")
        );
        let mut del_q = sqlx::query_scalar::<_, String>(&del_query).bind(id);
        for kid in &keep_image_ids {
            del_q = del_q.bind(kid);
        }
        del_q.fetch_all(&mut *tx).await.map_err(|e| e.to_string())?
    } else {
        let removed: Vec<(String,)> = sqlx::query_as(
            "DELETE FROM income_images WHERE record_id = ?1 RETURNING file_path",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        removed.into_iter().map(|(p,)| p).collect()
    };

    // Insert new images
    let mut saved_files: Vec<PathBuf> = Vec::new();

    let insert_result: Result<(), String> = async {
        for img in &new_images {
            let (relative_path, save_path) = save_image_file(&db, &img.file_bytes, &img.file_name)?;
            saved_files.push(save_path);

            sqlx::query(
                "INSERT INTO income_images (record_id, file_path, original_name, created_at) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(id)
            .bind(&relative_path)
            .bind(&img.file_name)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }.await;

    if let Err(e) = insert_result {
        tx.rollback().await.ok();
        for f in &saved_files { std::fs::remove_file(f).ok(); }
        return Err(e);
    }

    tx.commit().await.map_err(|e| {
        for f in &saved_files { std::fs::remove_file(f).ok(); }
        e.to_string()
    })?;

    // Only delete old image files after successful commit
    for path in &removed_paths {
        let full_path = resolve_image_path(&db, path);
        std::fs::remove_file(full_path).ok();
    }

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
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT file_path, original_name FROM income_images WHERE id = ?1",
    )
    .bind(image_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (relative_path, original_name) = row.ok_or("图片不存在")?;
    let full_path = resolve_image_path(&db, &relative_path);

    let bytes = std::fs::read(&full_path).map_err(|e| format!("读取图片失败: {}", e))?;

    let mime = mime_guess::from_path(&original_name)
        .first_or_octet_stream()
        .to_string();

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
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
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, record_id, file_path FROM income_images",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut orphans = Vec::new();
    for (id, record_id, file_path) in rows {
        let full_path = resolve_image_path(db, &file_path);
        if !full_path.exists() {
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
pub async fn cleanup_orphan_images(
    db: State<'_, DbState>,
    token: String,
) -> Result<u64, String> {
    db.validate_token(&token).await?;
    let orphans = do_check_image_consistency(&db).await?;
    let count = orphans.len() as u64;

    if count > 0 {
        let pool = db.get_pool().await?;
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        for orphan in &orphans {
            sqlx::query("DELETE FROM income_images WHERE id = ?1")
                .bind(orphan.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
    }

    Ok(count)
}
