use crate::db::DbState;
use crate::models::{BookRanking, DashboardStats, MonthlyIncome, MonthlySettlement};
use chrono::Datelike;
use tauri::State;

#[tauri::command]
pub async fn get_dashboard_stats(
    db: State<'_, DbState>,
    token: String,
    range_months: Option<i64>,
) -> Result<DashboardStats, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;
    let now = chrono::Local::now();
    let month_start = now.format("%Y-%m-01").to_string();

    // --- Merge 4 summary queries into a single CTE scan ---
    let summary: (Option<i64>, Option<i64>, i64, i64) = sqlx::query_as(
        r#"
        WITH stats AS (
            SELECT
                SUM(CASE WHEN date >= ?1 THEN total_amount ELSE 0 END) AS current_month,
                SUM(CASE WHEN settlement_status = 'unsettled' THEN total_amount ELSE 0 END) AS total_unsettled,
                COUNT(*) AS total_records,
                SUM(CASE WHEN settlement_status = 'unsettled' THEN 1 ELSE 0 END) AS pending
            FROM income_records
        )
        SELECT current_month, total_unsettled, total_records, pending FROM stats
        "#,
    )
    .bind(&month_start)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let ranking: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT b.id, b.name, SUM(r.total_amount) as unsettled_total
        FROM account_books b
        INNER JOIN income_records r ON r.book_id = b.id AND r.settlement_status = 'unsettled'
        GROUP BY b.id
        ORDER BY unsettled_total DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // Build recent N month keys and query trends (merged income + settlement into one scan)
    let months = range_months.unwrap_or(12).clamp(1, 60);
    let month_keys = build_recent_month_keys(months, &now);
    let trend_start = format!("{}-01", month_keys.first().unwrap());

    let trend_rows: Vec<(String, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT
            strftime('%Y-%m', date) AS month,
            SUM(total_amount) AS total_income,
            SUM(CASE WHEN settlement_status = 'settled' THEN total_amount ELSE 0 END) AS settled_amount,
            SUM(CASE WHEN settlement_status = 'unsettled' THEN total_amount ELSE 0 END) AS unsettled_amount
        FROM income_records
        WHERE date >= ?1
        GROUP BY strftime('%Y-%m', date)
        ORDER BY month
        "#,
    )
    .bind(&trend_start)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let trend_map: std::collections::HashMap<String, (i64, i64, i64)> = trend_rows
        .into_iter()
        .map(|(month, income, settled, unsettled)| {
            (month, (income.unwrap_or(0), settled.unwrap_or(0), unsettled.unwrap_or(0)))
        })
        .collect();

    let income_trend = month_keys
        .iter()
        .map(|month| MonthlyIncome {
            month: month.clone(),
            total_amount: trend_map.get(month).map(|(inc, _, _)| *inc).unwrap_or(0),
        })
        .collect();

    let settlement_trend = month_keys
        .iter()
        .map(|month| {
            let (_, settled, unsettled) = trend_map.get(month).copied().unwrap_or((0, 0, 0));
            MonthlySettlement {
                month: month.clone(),
                settled_amount: settled,
                unsettled_amount: unsettled,
            }
        })
        .collect();

    Ok(DashboardStats {
        current_month_income: summary.0.unwrap_or(0),
        total_unsettled: summary.1.unwrap_or(0),
        total_records: summary.2,
        pending_settlement: summary.3,
        book_ranking: ranking
            .into_iter()
            .map(|(book_id, book_name, amount)| BookRanking {
                book_id,
                book_name,
                unsettled_amount: amount.unwrap_or(0),
            })
            .collect(),
        income_trend,
        settlement_trend,
    })
}

fn build_recent_month_keys(months: i64, now: &chrono::DateTime<chrono::Local>) -> Vec<String> {
    let mut keys = Vec::with_capacity(months as usize);
    let mut year = now.year();
    let mut month = now.month() as i32;

    for _ in 0..months {
        keys.push(format!("{:04}-{:02}", year, month));
        month -= 1;
        if month == 0 {
            month = 12;
            year -= 1;
        }
    }

    keys.reverse();
    keys
}
