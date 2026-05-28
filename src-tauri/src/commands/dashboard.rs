use crate::db::DbState;
use crate::models::{BookRanking, CategoryShare, DashboardStats, MonthlyIncome, MonthlySettlement};
use chrono::Datelike;
use tauri::State;

#[tauri::command]
pub async fn get_dashboard_stats(db: State<'_, DbState>, token: String) -> Result<DashboardStats, String> {
    db.validate_token(&token).await?;
    let pool = db.get_pool().await?;
    let now = chrono::Local::now();
    let month_start = now.format("%Y-%m-01").to_string();

    let current_month: (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(total_amount) FROM income_records WHERE date >= ?1",
    )
    .bind(&month_start)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let total_unsettled: (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(total_amount) FROM income_records WHERE settlement_status = 'unsettled'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let total_records: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM income_records")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let pending: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_records WHERE settlement_status = 'unsettled'")
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

    // Build recent 12 month keys and query trends
    let month_keys = build_recent_month_keys(12, &now);
    let month_start_12 = format!("{}-01", month_keys.first().unwrap());

    let income_rows: Vec<(String, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT strftime('%Y-%m', date) as month, SUM(total_amount)
        FROM income_records
        WHERE date >= ?1
        GROUP BY strftime('%Y-%m', date)
        ORDER BY month
        "#,
    )
    .bind(&month_start_12)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let settlement_rows: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT
            strftime('%Y-%m', date) as month,
            SUM(CASE WHEN settlement_status = 'settled' THEN total_amount ELSE 0 END) as settled_amount,
            SUM(CASE WHEN settlement_status = 'unsettled' THEN total_amount ELSE 0 END) as unsettled_amount
        FROM income_records
        WHERE date >= ?1
        GROUP BY strftime('%Y-%m', date)
        ORDER BY month
        "#,
    )
    .bind(&month_start_12)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let category_rows: Vec<(String, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT category, SUM(total_amount)
        FROM income_records
        WHERE date >= ?1
        GROUP BY category
        ORDER BY SUM(total_amount) DESC
        "#,
    )
    .bind(&month_start_12)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let income_map: std::collections::HashMap<String, i64> = income_rows
        .into_iter()
        .map(|(month, amount)| (month, amount.unwrap_or(0)))
        .collect();

    let settlement_map: std::collections::HashMap<String, (i64, i64)> = settlement_rows
        .into_iter()
        .map(|(month, settled, unsettled)| (month, (settled.unwrap_or(0), unsettled.unwrap_or(0))))
        .collect();

    let income_trend = month_keys
        .iter()
        .map(|month| MonthlyIncome {
            month: month.clone(),
            total_amount: income_map.get(month).copied().unwrap_or(0),
        })
        .collect();

    let settlement_trend = month_keys
        .iter()
        .map(|month| {
            let (settled, unsettled) = settlement_map.get(month).copied().unwrap_or((0, 0));
            MonthlySettlement {
                month: month.clone(),
                settled_amount: settled,
                unsettled_amount: unsettled,
            }
        })
        .collect();

    let category_share = category_rows
        .into_iter()
        .map(|(category, amount)| CategoryShare {
            category,
            amount: amount.unwrap_or(0),
        })
        .collect();

    Ok(DashboardStats {
        current_month_income: current_month.0.unwrap_or(0),
        total_unsettled: total_unsettled.0.unwrap_or(0),
        total_records: total_records.0,
        pending_settlement: pending.0,
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
        category_share,
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
