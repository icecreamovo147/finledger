use crate::commands::record::resolve_image_path;
use crate::db::DbState;
use crate::models::AccountBook;
use tauri::State;

#[tauri::command]
pub async fn create_book(
    db: State<'_, DbState>,
    name: String,
    remark: Option<String>,
) -> Result<AccountBook, String> {
    if name.trim().is_empty() {
        return Err("账本名称不能为空".into());
    }

    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let id = sqlx::query(
        "INSERT INTO account_books (name, remark, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(name.trim())
    .bind(&remark)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();

    Ok(AccountBook {
        id,
        name: name.trim().to_string(),
        remark,
        created_at: now.clone(),
        updated_at: now,
        total_unsettled: Some(0.0),
        record_count: Some(0),
    })
}

#[tauri::command]
pub async fn list_books(db: State<'_, DbState>) -> Result<Vec<AccountBook>, String> {
    let books: Vec<(i64, String, String, String, String, Option<f64>, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT
            b.id, b.name, b.remark, b.created_at, b.updated_at,
            COALESCE(SUM(CASE WHEN r.settlement_status = 'unsettled' THEN r.total_amount ELSE 0.0 END), 0.0) as total_unsettled,
            COUNT(r.id) as record_count
        FROM account_books b
        LEFT JOIN income_records r ON r.book_id = b.id
        GROUP BY b.id
        ORDER BY b.updated_at DESC
        "#,
    )
    .fetch_all(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(books
        .into_iter()
        .map(|(id, name, remark, created_at, updated_at, total_unsettled, record_count)| {
            AccountBook {
                id,
                name,
                remark,
                created_at,
                updated_at,
                total_unsettled,
                record_count,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn update_book(
    db: State<'_, DbState>,
    id: i64,
    name: String,
    remark: Option<String>,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("账本名称不能为空".into());
    }

    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let result = sqlx::query(
        "UPDATE account_books SET name = ?1, remark = ?2, updated_at = ?3 WHERE id = ?4",
    )
    .bind(name.trim())
    .bind(&remark)
    .bind(&now)
    .bind(id)
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("账本不存在".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_book(db: State<'_, DbState>, id: i64) -> Result<(), String> {
    // Delete all associated image files
    let images: Vec<(String,)> = sqlx::query_as(
        "SELECT file_path FROM income_images WHERE record_id IN (SELECT id FROM income_records WHERE book_id = ?1)",
    )
    .bind(id)
    .fetch_all(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    for (path,) in &images {
        let full_path = resolve_image_path(&db, path);
        std::fs::remove_file(full_path).ok();
    }

    // Cascade delete: images and records are deleted via FK cascade
    let result = sqlx::query("DELETE FROM account_books WHERE id = ?1")
        .bind(id)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("账本不存在".into());
    }
    Ok(())
}
