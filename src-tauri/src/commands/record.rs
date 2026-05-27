use crate::db::DbState;
use crate::models::{IncomeImage, IncomeRecord, PaginatedRecords};
use base64::Engine;
use std::path::{Path, PathBuf};
use tauri::State;
use uuid::Uuid;

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

#[tauri::command]
pub async fn create_record(
    db: State<'_, DbState>,
    book_id: i64,
    date: String,
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<f64>,
    size_info: Option<String>,
    total_amount: f64,
    remark: Option<String>,
) -> Result<IncomeRecord, String> {
    if date.is_empty() {
        return Err("日期不能为空".into());
    }
    if category.is_empty() {
        return Err("类别不能为空".into());
    }
    if total_amount < 0.0 {
        return Err("金额不能为负数".into());
    }

    let description = description.unwrap_or_default();
    let unit = unit.unwrap_or_default();
    let size_info = size_info.unwrap_or_default();
    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

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
    .execute(&db.pool)
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
    book_id: i64,
    category: Option<String>,
    settlement_status: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedRecords, String> {
    let mut query_str = String::from(
        "SELECT id, book_id, date, category, description, quantity, unit, unit_price, size_info, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE book_id = ?1",
    );

    if category.is_some() {
        query_str.push_str(" AND category = ?2");
    }
    if settlement_status.is_some() {
        let idx = if category.is_some() { "?3" } else { "?2" };
        query_str.push_str(&format!(" AND settlement_status = {}", idx));
    }
    if date_from.is_some() {
        let idx = 2 + category.is_some() as u8 + settlement_status.is_some() as u8;
        query_str.push_str(&format!(" AND date >= ?{}", idx));
    }
    if date_to.is_some() {
        let idx = 2 + category.is_some() as u8 + settlement_status.is_some() as u8 + date_from.is_some() as u8;
        query_str.push_str(&format!(" AND date <= ?{}", idx));
    }
    if keyword.is_some() {
        let idx = 2 + category.is_some() as u8 + settlement_status.is_some() as u8 + date_from.is_some() as u8 + date_to.is_some() as u8;
        query_str.push_str(&format!(" AND (description LIKE ?{} OR remark LIKE ?{})", idx, idx));
    }

    query_str.push_str(" ORDER BY date DESC, id DESC");

    // Build query with dynamic bindings
    // For simplicity, we fetch all records for the book and filter in Rust
    // A production system would use a query builder
    let records: Vec<(
        i64, i64, String, String, String, Option<i64>, String, Option<f64>, String,
        f64, String, Option<String>, Option<String>, String, String, String,
    )> = sqlx::query_as(&query_str)
        .bind(book_id)
        .fetch_all(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut result: Vec<IncomeRecord> = records
        .into_iter()
        .map(
            |(
                id, book_id, date, category, description, quantity, unit, unit_price, size_info,
                total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at,
            )| {
                IncomeRecord {
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
                    images: vec![],
                    created_at,
                    updated_at,
                }
            },
        )
        .collect();

    // Apply filters manually since dynamic SQL bindings are complex
    if let Some(cat) = &category {
        result.retain(|r| &r.category == cat);
    }
    if let Some(status) = &settlement_status {
        result.retain(|r| &r.settlement_status == status);
    }
    if let Some(from) = &date_from {
        result.retain(|r| &r.date >= from);
    }
    if let Some(to) = &date_to {
        result.retain(|r| &r.date <= to);
    }
    if let Some(kw) = &keyword {
        let kw = kw.to_lowercase();
        result.retain(|r| {
            r.description.to_lowercase().contains(&kw)
                || r.remark.to_lowercase().contains(&kw)
        });
    }

    let total = result.len() as i64;
    let total_unsettled: f64 = result
        .iter()
        .filter(|r| r.settlement_status == "unsettled")
        .map(|r| r.total_amount)
        .sum();

    if let (Some(p), Some(ps)) = (page, page_size) {
        let offset = (p - 1).max(0) as usize * ps as usize;
        result = result.into_iter().skip(offset).take(ps as usize).collect();
    }

    Ok(PaginatedRecords {
        total,
        total_unsettled,
        records: result,
    })
}

#[tauri::command]
pub async fn get_record(db: State<'_, DbState>, id: i64) -> Result<IncomeRecord, String> {
    let row: Option<(
        i64, i64, String, String, String, Option<i64>, String, Option<f64>, String,
        f64, String, Option<String>, Option<String>, String, String, String,
    )> = sqlx::query_as(
        "SELECT id, book_id, date, category, description, quantity, unit, unit_price, size_info, \
         total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at \
         FROM income_records WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    let (
        id, book_id, date, category, description, quantity, unit, unit_price, size_info,
        total_amount, settlement_status, payment_date, payment_method, remark, created_at, updated_at,
    ) = row.ok_or("记录不存在")?;

    // Fetch images
    let images: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, record_id, file_path, original_name, created_at FROM income_images WHERE record_id = ?1",
    )
    .bind(id)
    .fetch_all(&db.pool)
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
    id: i64,
    date: String,
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit: Option<String>,
    unit_price: Option<f64>,
    size_info: Option<String>,
    total_amount: f64,
    remark: Option<String>,
) -> Result<(), String> {
    // Check if settled
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        return Err("已结清记录不可修改".into());
    }

    if total_amount < 0.0 {
        return Err("金额不能为负数".into());
    }

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
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_record(db: State<'_, DbState>, id: i64) -> Result<(), String> {
    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    let (current_status,) = status.ok_or("记录不存在")?;
    if current_status == "settled" {
        return Err("已结清记录不可删除".into());
    }

    // Delete associated image files
    let images: Vec<(String,)> =
        sqlx::query_as("SELECT file_path FROM income_images WHERE record_id = ?1")
            .bind(id)
            .fetch_all(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    for (path,) in &images {
        let full_path = resolve_image_path(&db, path);
        std::fs::remove_file(full_path).ok();
    }

    sqlx::query("DELETE FROM income_records WHERE id = ?1")
        .bind(id)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ===== T11: Image Upload/Delete =====

#[tauri::command]
pub async fn upload_image(
    db: State<'_, DbState>,
    record_id: i64,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<IncomeImage, String> {
    if file_bytes.len() > 20 * 1024 * 1024 {
        return Err("图片大小不能超过 20MB".into());
    }

    let ext = Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let new_name = format!("{}.{}", Uuid::new_v4(), ext);

    std::fs::create_dir_all(&db.images_dir).ok();
    let save_path = db.images_dir.join(&new_name);
    std::fs::write(&save_path, &file_bytes).map_err(|e| e.to_string())?;

    let relative_path = format!("images/{}", new_name); // Always store relative path with forward slash
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let id = sqlx::query(
        "INSERT INTO income_images (record_id, file_path, original_name, created_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(record_id)
    .bind(&relative_path)
    .bind(&file_name)
    .bind(&now)
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();

    Ok(IncomeImage {
        id,
        record_id,
        file_path: relative_path,
        original_name: file_name,
        created_at: now,
    })
}

#[tauri::command]
pub async fn delete_image(db: State<'_, DbState>, id: i64) -> Result<(), String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT file_path FROM income_images WHERE id = ?1")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some((path,)) = row {
        let full_path = resolve_image_path(&db, &path);
        std::fs::remove_file(full_path).ok();
    }

    sqlx::query("DELETE FROM income_images WHERE id = ?1")
        .bind(id)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ===== T14: Settlement =====

#[tauri::command]
pub async fn settle_record(
    db: State<'_, DbState>,
    id: i64,
    payment_date: String,
    payment_method: String,
) -> Result<(), String> {
    if payment_date.is_empty() {
        return Err("收款日期不能为空".into());
    }
    if payment_method.is_empty() {
        return Err("收款方式不能为空".into());
    }

    let status: Option<(String,)> =
        sqlx::query_as("SELECT settlement_status FROM income_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&db.pool)
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
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn unsettle_record(db: State<'_, DbState>, id: i64) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query(
        r#"UPDATE income_records SET
        settlement_status = 'unsettled', payment_date = NULL, payment_method = NULL, updated_at = ?1
        WHERE id = ?2"#,
    )
    .bind(&now)
    .bind(id)
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn read_image_base64(
    db: State<'_, DbState>,
    image_id: i64,
) -> Result<String, String> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT file_path, original_name FROM income_images WHERE id = ?1",
    )
    .bind(image_id)
    .fetch_optional(&db.pool)
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
