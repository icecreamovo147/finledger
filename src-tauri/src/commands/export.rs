use crate::db::DbState;
use tracing::info;
use rust_xlsxwriter::{Format, FormatAlign, Image, Workbook};
use tauri::State;

type RecordRow = (
    i64,
    String,
    String,
    String,
    Option<i64>,
    String,
    Option<i64>,
    i64,
    String,
);

async fn write_export_sheet(
    db: &crate::db::DbState,
    workbook: &mut Workbook,
    rows: &[RecordRow],
    pool: &sqlx::SqlitePool,
) -> Result<(), String> {
    let sheet = workbook.add_worksheet();

    let header_fmt = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color(0x409EFF)
        .set_font_color(0xFFFFFF);

    let money_fmt = Format::new().set_num_format("#,##0.00");

    let text_fmt = Format::new()
        .set_text_wrap()
        .set_align(FormatAlign::Top);

    let headers = [
        "日期",
        "服务项目及内容",
        "规格",
        "数量",
        "单位",
        "单价（元）",
        "总金额（元）",
        "备注",
        "图片",
    ];
    for (col, header) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, header.to_string(), &header_fmt)
            .map_err(|e| e.to_string())?;
    }

    macro_rules! set_width {
        ($sheet:expr, $col:expr, $w:expr) => {
            $sheet
                .set_column_width($col, $w)
                .map_err(|e| e.to_string())?;
        };
    }
    set_width!(sheet, 0, 14);
    set_width!(sheet, 1, 28);
    set_width!(sheet, 2, 22);
    set_width!(sheet, 3, 8);
    set_width!(sheet, 4, 6);
    set_width!(sheet, 5, 10);
    set_width!(sheet, 6, 12);
    set_width!(sheet, 7, 24);
    set_width!(sheet, 8, 34);

    let mut total_cents: i64 = 0;

    for (
        row_idx,
        (
            record_id,
            date,
            service_content,
            specification,
            quantity,
            unit,
            unit_price,
            amount,
            remark,
        ),
    ) in rows.iter().enumerate()
    {
        let r: u32 = (row_idx + 1) as u32;

        sheet.write_string(r, 0, date).map_err(|e| e.to_string())?;
        sheet
            .write_string_with_format(r, 1, service_content, &text_fmt)
            .map_err(|e| e.to_string())?;
        sheet
            .write_string_with_format(r, 2, specification, &text_fmt)
            .map_err(|e| e.to_string())?;
        if let Some(qty) = quantity {
            sheet
                .write_number(r, 3, *qty as f64)
                .map_err(|e| e.to_string())?;
        }
        sheet.write_string(r, 4, unit).map_err(|e| e.to_string())?;
        if let Some(price) = unit_price {
            sheet
                .write_number_with_format(r, 5, *price as f64 / 100.0, &money_fmt)
                .map_err(|e| e.to_string())?;
        }
        sheet
            .write_number_with_format(r, 6, *amount as f64 / 100.0, &money_fmt)
            .map_err(|e| e.to_string())?;
        sheet
            .write_string_with_format(r, 7, remark, &text_fmt)
            .map_err(|e| e.to_string())?;

        let max_newlines = [
            service_content.matches('\n').count(),
            specification.matches('\n').count(),
            remark.matches('\n').count(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        let text_height = (max_newlines + 1) as f64 * 15.0;

        let images: Vec<(String,)> =
            sqlx::query_as("SELECT file_path FROM income_images WHERE record_id = ?1 ORDER BY id")
                .bind(record_id)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;

        if !images.is_empty() {
            let image_rows = ((images.len() + 2) / 3) as f64;
            sheet
                .set_row_height(r, f64::max(text_height, image_rows * 58.0))
                .map_err(|e| e.to_string())?;

            let mut embedded_count = 0;
            for (img_idx, (file_path,)) in images.iter().enumerate() {
                let full_path = match crate::commands::record::resolve_image_path(db, file_path) {
                    Ok(p) if p.exists() => p,
                    _ => continue,
                };
                {
                    let img = Image::new(full_path)
                        .map_err(|e| e.to_string())?
                        .set_scale_to_size(64, 64, true);
                    let x_offset = 6 + ((img_idx % 3) as u32 * 72);
                    let y_offset = 6 + ((img_idx / 3) as u32 * 72);
                    sheet
                        .insert_image_with_offset(r, 8, &img, x_offset, y_offset)
                        .map_err(|e| e.to_string())?;
                    embedded_count += 1;
                }
            }

            if embedded_count == 0 {
                sheet
                    .write_string(r, 8, "图片文件缺失")
                    .map_err(|e| e.to_string())?;
            }
        } else {
            if max_newlines > 0 {
                sheet
                    .set_row_height(r, text_height)
                    .map_err(|e| e.to_string())?;
            }
            sheet.write_string(r, 8, "-").map_err(|e| e.to_string())?;
        }

        total_cents += amount;
    }

    let total_row: u32 = (rows.len() + 1) as u32;
    let total_label_fmt = Format::new().set_bold();
    sheet
        .write_string_with_format(total_row, 5, "合计：", &total_label_fmt)
        .map_err(|e| e.to_string())?;
    sheet
        .write_number_with_format(
            total_row,
            6,
            total_cents as f64 / 100.0,
            &Format::new().set_bold().set_num_format("#,##0.00"),
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ===== Internal helpers (take &DbState, testable without Tauri) =====

pub async fn do_export_excel(
    db: &DbState,
    book_id: i64,
    record_ids: Vec<i64>,
    save_path: &str,
) -> Result<String, String> {
    if record_ids.is_empty() {
        return Err("没有选择任何记录".into());
    }
    let pool = db.get_pool().await?;

    // Only unsettled records
    let placeholders: Vec<String> = (0..record_ids.len())
        .map(|i| format!("?{}", i + 2))
        .collect();
    let check_q = format!(
        "SELECT COUNT(*) FROM income_records WHERE book_id = ?1 AND id IN ({}) AND settlement_status = 'settled'",
        placeholders.join(", ")
    );
    let mut cq = sqlx::query_as::<_, (i64,)>(&check_q).bind(book_id);
    for id in &record_ids {
        cq = cq.bind(id);
    }
    if cq.fetch_one(&pool).await.map_err(|e| e.to_string())?.0 > 0 {
        return Err("只能导出未结清记录".into());
    }

    // Verify existence
    let exist_q = format!(
        "SELECT COUNT(*) FROM income_records WHERE book_id = ?1 AND id IN ({})",
        placeholders.join(", ")
    );
    let mut eq = sqlx::query_as::<_, (i64,)>(&exist_q).bind(book_id);
    for id in &record_ids {
        eq = eq.bind(id);
    }
    if eq.fetch_one(&pool).await.map_err(|e| e.to_string())?.0 != record_ids.len() as i64 {
        return Err("部分记录不存在或不属于该账本".into());
    }

    // Fetch rows
    let data_q = format!(
        "SELECT id, date, service_content, specification, quantity, unit, unit_price, total_amount, remark \
         FROM income_records WHERE book_id = ?1 AND id IN ({}) ORDER BY date ASC, id ASC",
        placeholders.join(", ")
    );
    let mut dq = sqlx::query_as::<_, RecordRow>(&data_q).bind(book_id);
    for id in &record_ids {
        dq = dq.bind(id);
    }
    let rows = dq.fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let mut workbook = Workbook::new();
    write_export_sheet(db, &mut workbook, &rows, &pool).await?;
    workbook.save(save_path).map_err(|e| e.to_string())?;
    info!("导出完成: {} 条记录 -> {}", rows.len(), save_path);
    Ok(save_path.to_string())
}

pub async fn do_export_all_unsettled(
    db: &DbState,
    book_id: i64,
    save_path: &str,
) -> Result<String, String> {
    let pool = db.get_pool().await?;
    let rows: Vec<RecordRow> = sqlx::query_as(
        "SELECT id, date, service_content, specification, quantity, unit, unit_price, total_amount, remark \
         FROM income_records WHERE book_id = ?1 AND settlement_status = 'unsettled' ORDER BY date ASC, id ASC",
    )
    .bind(book_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Err("该账本没有未结清记录".into());
    }

    let mut workbook = Workbook::new();
    write_export_sheet(db, &mut workbook, &rows, &pool).await?;
    workbook.save(save_path).map_err(|e| e.to_string())?;
    Ok(save_path.to_string())
}

// ===== Tauri command wrappers =====

#[tauri::command]
pub async fn export_excel(
    db: State<'_, DbState>,
    token: String,
    book_id: i64,
    record_ids: Vec<i64>,
    save_path: String,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    do_export_excel(&db, book_id, record_ids, &save_path).await
}

#[tauri::command]
pub async fn export_all_unsettled(
    db: State<'_, DbState>,
    token: String,
    book_id: i64,
    save_path: String,
) -> Result<String, String> {
    db.validate_token(&token).await?;
    do_export_all_unsettled(&db, book_id, &save_path).await
}
