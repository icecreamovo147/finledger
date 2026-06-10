use crate::commands::record::resolve_image_path;
use crate::db::DbState;
use tauri::State;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OrphanImage {
    pub id: i64,
    pub record_id: i64,
    pub file_path: String,
}

/// 分批检查附件一致性，避免大数据量下全表扫描导致内存压力
pub async fn do_check_image_consistency(db: &DbState) -> Result<Vec<OrphanImage>, String> {
    let pool = db.get_pool().await?;

    // 使用分页查询，每次处理 500 条，避免一次性加载全部记录到内存
    const BATCH_SIZE: i64 = 500;
    let mut offset = 0i64;
    let mut orphans = Vec::new();

    loop {
        let rows: Vec<(i64, i64, String)> = sqlx::query_as(
            "SELECT id, record_id, file_path FROM income_images ORDER BY id LIMIT ?1 OFFSET ?2",
        )
        .bind(BATCH_SIZE)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            break;
        }

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

        offset += BATCH_SIZE;
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
    // 同样使用分页避免内存压力
    const BATCH_SIZE: i64 = 500;
    let mut offset = 0i64;
    let mut orphan_ids = Vec::new();

    loop {
        let rows: Vec<(i64, i64, String)> = sqlx::query_as(
            "SELECT id, record_id, file_path FROM income_images ORDER BY id LIMIT ?1 OFFSET ?2",
        )
        .bind(BATCH_SIZE)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            break;
        }

        for (id, _record_id, file_path) in &rows {
            let is_orphan = match resolve_image_path(&db, file_path) {
                Ok(full_path) => !full_path.exists(),
                Err(_) => true,
            };
            if is_orphan {
                orphan_ids.push(*id);
            }
        }

        offset += BATCH_SIZE;
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
