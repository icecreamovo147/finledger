use crate::db::DbState;
use rust_xlsxwriter::{Format, Image, Workbook};
use tauri::State;

#[tauri::command]
pub async fn export_excel(
    db: State<'_, DbState>,
    book_id: i64,
    record_ids: Vec<i64>,
    save_path: String,
) -> Result<String, String> {
    if record_ids.is_empty() {
        return Err("没有选择任何记录".into());
    }

    let placeholders: Vec<String> = record_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 2))
        .collect();
    let query = format!(
        "SELECT id, date, category, description, quantity, unit, unit_price, size_info, total_amount, remark \
         FROM income_records WHERE book_id = ?1 AND id IN ({}) ORDER BY date ASC, id ASC",
        placeholders.join(", ")
    );

    let mut q = sqlx::query_as::<_, (i64, String, String, String, Option<i64>, String, Option<f64>, String, f64, String)>(&query)
        .bind(book_id);

    for id in &record_ids {
        q = q.bind(id);
    }

    let rows = q.fetch_all(&db.pool).await.map_err(|e| e.to_string())?;

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    let header_fmt = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color(0x409EFF)
        .set_font_color(0xFFFFFF);

    let money_fmt = Format::new().set_num_format("#,##0.00");

    let headers = ["日期", "类别", "描述", "数量", "单位", "单价", "尺寸", "总金额", "备注", "图片"];
    for (col, header) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, header.to_string(), &header_fmt)
            .map_err(|e| e.to_string())?;
    }

    macro_rules! set_width {
        ($sheet:expr, $col:expr, $w:expr) => {
            $sheet.set_column_width($col, $w).map_err(|e| e.to_string())?;
        };
    }
    set_width!(sheet, 0, 14);
    set_width!(sheet, 1, 12);
    set_width!(sheet, 2, 26);
    set_width!(sheet, 3, 8);
    set_width!(sheet, 4, 6);
    set_width!(sheet, 5, 10);
    set_width!(sheet, 6, 14);
    set_width!(sheet, 7, 12);
    set_width!(sheet, 8, 24);
    set_width!(sheet, 9, 34);

    let category_labels: std::collections::HashMap<&str, &str> = [
        ("Print", "打印"),
        ("Copy", "复印"),
        ("Binding", "装订"),
        ("PostProcess", "后加工"),
        ("Design", "广告设计费"),
        ("MaterialProd", "物料制作"),
        ("AdRental", "广告位租赁"),
        ("AdAgency", "代理投放"),
        ("Installation", "安装费"),
        ("Other", "其他"),
    ]
    .into_iter()
    .collect();

    let mut total = 0.0;

    for (row_idx, (record_id, date, category, description, quantity, unit, unit_price, size_info, amount, remark)) in
        rows.iter().enumerate()
    {
        let r: u32 = (row_idx + 1) as u32;
        let cat_label = category_labels
            .get(category.as_str())
            .copied()
            .unwrap_or(category.as_str());

        sheet.write_string(r, 0, date).map_err(|e| e.to_string())?;
        sheet.write_string(r, 1, cat_label).map_err(|e| e.to_string())?;
        sheet.write_string(r, 2, description).map_err(|e| e.to_string())?;
        if let Some(qty) = quantity {
            sheet.write_number(r, 3, *qty as f64).map_err(|e| e.to_string())?;
        }
        sheet.write_string(r, 4, unit).map_err(|e| e.to_string())?;
        if let Some(price) = unit_price {
            sheet.write_number_with_format(r, 5, *price, &money_fmt).map_err(|e| e.to_string())?;
        }
        sheet.write_string(r, 6, size_info).map_err(|e| e.to_string())?;
        sheet
            .write_number_with_format(r, 7, *amount, &money_fmt)
            .map_err(|e| e.to_string())?;
        sheet.write_string(r, 8, remark).map_err(|e| e.to_string())?;

        // Fetch and embed images for this record
        let images: Vec<(String,)> = sqlx::query_as(
            "SELECT file_path FROM income_images WHERE record_id = ?1 ORDER BY id",
        )
        .bind(record_id)
        .fetch_all(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

        if !images.is_empty() {
            let image_rows = ((images.len() + 2) / 3) as f64;
            sheet.set_row_height(r, image_rows * 58.0).map_err(|e| e.to_string())?;

            let mut embedded_count = 0;
            for (img_idx, (file_path,)) in images.iter().enumerate() {
                let full_path = crate::commands::record::resolve_image_path(&db, file_path);
                if full_path.exists() {
                    let img = Image::new(full_path)
                        .map_err(|e| e.to_string())?
                        .set_scale_to_size(64, 64, true);
                    let x_offset = 6 + ((img_idx % 3) as u32 * 72);
                    let y_offset = 6 + ((img_idx / 3) as u32 * 72);
                    sheet
                        .insert_image_with_offset(r, 9, &img, x_offset, y_offset)
                        .map_err(|e| e.to_string())?;
                    embedded_count += 1;
                }
            }

            if embedded_count == 0 {
                sheet.write_string(r, 9, "图片文件缺失").map_err(|e| e.to_string())?;
            }
        } else {
            sheet.write_string(r, 9, "-").map_err(|e| e.to_string())?;
        }

        total += amount;
    }

    let total_row: u32 = (rows.len() + 1) as u32;
    let total_label_fmt = Format::new().set_bold();
    sheet
        .write_string_with_format(total_row, 6, "合计：", &total_label_fmt)
        .map_err(|e| e.to_string())?;
    sheet
        .write_number_with_format(
            total_row,
            7,
            total,
            &Format::new().set_bold().set_num_format("#,##0.00"),
        )
        .map_err(|e| e.to_string())?;

    workbook.save(&save_path).map_err(|e| e.to_string())?;

    Ok(save_path)
}
