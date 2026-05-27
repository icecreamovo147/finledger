use crate::db::DbState;
use crate::models::{BookRanking, DashboardStats};
use tauri::State;

#[tauri::command]
pub async fn get_dashboard_stats(db: State<'_, DbState>) -> Result<DashboardStats, String> {
    let now = chrono::Local::now();
    let month_start = now.format("%Y-%m-01").to_string();

    // Current month income (total, including both settled and unsettled)
    let current_month: (Option<f64>,) = sqlx::query_as(
        "SELECT SUM(total_amount) FROM income_records WHERE date >= ?1",
    )
    .bind(&month_start)
    .fetch_one(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    // Total unsettled
    let total_unsettled: (Option<f64>,) = sqlx::query_as(
        "SELECT SUM(total_amount) FROM income_records WHERE settlement_status = 'unsettled'",
    )
    .fetch_one(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    // Total records
    let total_records: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM income_records")
        .fetch_one(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    // Pending settlement count
    let pending: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_records WHERE settlement_status = 'unsettled'")
            .fetch_one(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    // Book ranking by unsettled amount
    let ranking: Vec<(i64, String, Option<f64>)> = sqlx::query_as(
        r#"
        SELECT b.id, b.name, SUM(r.total_amount) as unsettled_total
        FROM account_books b
        INNER JOIN income_records r ON r.book_id = b.id AND r.settlement_status = 'unsettled'
        GROUP BY b.id
        ORDER BY unsettled_total DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(DashboardStats {
        current_month_income: current_month.0.unwrap_or(0.0),
        total_unsettled: total_unsettled.0.unwrap_or(0.0),
        total_records: total_records.0,
        pending_settlement: pending.0,
        book_ranking: ranking
            .into_iter()
            .map(|(book_id, book_name, amount)| BookRanking {
                book_id,
                book_name,
                unsettled_amount: amount.unwrap_or(0.0),
            })
            .collect(),
    })
}
