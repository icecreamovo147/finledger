use crate::commands::record::resolve_image_path;
use tracing::warn;
use crate::db::DbState;
use crate::models::{AccountBook, PaginatedBooks};
use crate::utils::escape_like;
use tauri::State;

#[tauri::command]
pub async fn create_book(
    db: State<'_, DbState>,
    token: String,
    name: String,
    remark: Option<String>,
) -> Result<AccountBook, String> {
    db.validate_token(&token).await?;
    if name.trim().is_empty() {
        return Err("账本名称不能为空".into());
    }

    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let pool = db.get_pool().await?;

    let id = sqlx::query(
        "INSERT INTO account_books (name, remark, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(name.trim())
    .bind(&remark)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "账本名称已存在".into()
        } else {
            e.to_string()
        }
    })?
    .last_insert_rowid();

    Ok(AccountBook {
        id,
        name: name.trim().to_string(),
        remark,
        created_at: now.clone(),
        updated_at: now,
        total_unsettled: Some(0),
        record_count: Some(0),
    })
}

#[tauri::command]
pub async fn list_books(
    db: State<'_, DbState>,
    token: String,
    page: Option<i64>,
    page_size: Option<i64>,
    keyword: Option<String>,
) -> Result<PaginatedBooks, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let keyword = keyword.filter(|k| !k.trim().is_empty());
    let search_pattern = keyword.as_ref().map(|k| format!("%{}%", escape_like(k.trim())));

    let total: (i64,) = if search_pattern.is_some() {
        sqlx::query_as(
            "SELECT COUNT(*) FROM account_books WHERE name LIKE ?1 ESCAPE '\\' OR remark LIKE ?1 ESCAPE '\\'",
        )
        .bind(search_pattern.as_ref().unwrap())
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM account_books")
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?
    };

    let books: Vec<(i64, String, String, String, String, Option<i64>, Option<i64>)> =
        if let Some(ref pat) = search_pattern {
            sqlx::query_as(
                "SELECT \
                    b.id, b.name, b.remark, b.created_at, b.updated_at, \
                    COALESCE(SUM(CASE WHEN r.settlement_status = 'unsettled' THEN r.total_amount ELSE 0 END), 0) as total_unsettled, \
                    COUNT(r.id) as record_count \
                FROM account_books b \
                LEFT JOIN income_records r ON r.book_id = b.id \
                WHERE b.name LIKE ?1 ESCAPE '\\' OR b.remark LIKE ?1 ESCAPE '\\' \
                GROUP BY b.id \
                ORDER BY b.created_at ASC \
                LIMIT ?2 OFFSET ?3",
            )
            .bind(pat)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    b.id, b.name, b.remark, b.created_at, b.updated_at,
                    COALESCE(SUM(CASE WHEN r.settlement_status = 'unsettled' THEN r.total_amount ELSE 0 END), 0) as total_unsettled,
                    COUNT(r.id) as record_count
                FROM account_books b
                LEFT JOIN income_records r ON r.book_id = b.id
                GROUP BY b.id
                ORDER BY b.created_at ASC
                LIMIT ?1 OFFSET ?2
                "#,
            )
            .bind(page_size)
            .bind(offset)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?
        };

    Ok(PaginatedBooks {
        total: total.0,
        books: books
            .into_iter()
            .map(
                |(id, name, remark, created_at, updated_at, total_unsettled, record_count)| {
                    AccountBook {
                        id,
                        name,
                        remark,
                        created_at,
                        updated_at,
                        total_unsettled,
                        record_count,
                    }
                },
            )
            .collect(),
    })
}

#[tauri::command]
pub async fn get_book(
    db: State<'_, DbState>,
    token: String,
    id: i64,
) -> Result<AccountBook, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    let book: Option<(i64, String, String, String, String, Option<i64>, Option<i64>)> =
        sqlx::query_as(
            r#"
            SELECT
                b.id, b.name, b.remark, b.created_at, b.updated_at,
                COALESCE(SUM(CASE WHEN r.settlement_status = 'unsettled' THEN r.total_amount ELSE 0 END), 0) as total_unsettled,
                COUNT(r.id) as record_count
            FROM account_books b
            LEFT JOIN income_records r ON r.book_id = b.id
            WHERE b.id = ?1
            GROUP BY b.id
            "#,
        )
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    book.map(
        |(id, name, remark, created_at, updated_at, total_unsettled, record_count)| AccountBook {
            id,
            name,
            remark,
            created_at,
            updated_at,
            total_unsettled,
            record_count,
        },
    )
    .ok_or_else(|| "账本不存在".into())
}

#[tauri::command]
pub async fn update_book(
    db: State<'_, DbState>,
    token: String,
    id: i64,
    name: String,
    remark: Option<String>,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    if name.trim().is_empty() {
        return Err("账本名称不能为空".into());
    }

    let remark = remark.unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let pool = db.get_pool().await?;

    let result = sqlx::query(
        "UPDATE account_books SET name = ?1, remark = ?2, updated_at = ?3 WHERE id = ?4",
    )
    .bind(name.trim())
    .bind(&remark)
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "账本名称已存在".into()
        } else {
            e.to_string()
        }
    })?;

    if result.rows_affected() == 0 {
        return Err("账本不存在".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_book(db: State<'_, DbState>, token: String, id: i64) -> Result<(), String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;

    // Begin transaction first to prevent TOCTOU race between image query and delete
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // Query all associated image paths within the transaction
    let images: Vec<(String,)> = sqlx::query_as(
        "SELECT file_path FROM income_images WHERE record_id IN (SELECT id FROM income_records WHERE book_id = ?1)",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // Delete the book (FK cascade deletes records and images)
    let result = sqlx::query("DELETE FROM account_books WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        tx.rollback().await.ok();
        return Err("账本不存在".into());
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    // After commit, delete image files (best-effort)
    for (path,) in &images {
        if let Ok(full_path) = resolve_image_path(&db, path) {
            if let Err(e) = std::fs::remove_file(&full_path) {
                warn!("警告: 无法删除图片文件 {}: {}", full_path.display(), e);
            }
        }
    }
    Ok(())
}
