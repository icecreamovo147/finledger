use finledger_lib::commands::{auth, export, record};
use finledger_lib::db::DbState;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

async fn setup_db() -> DbState {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let db = DbState::new(pool, PathBuf::from("/tmp/finledger_test"));
    db.run_migrations().await.unwrap();
    db
}

async fn create_session(db: &DbState, user_id: i64) -> String {
    let token = uuid::Uuid::new_v4().to_string();
    let expires = (chrono::Utc::now() + chrono::Duration::hours(1))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let pool = db.raw_pool().await;
    sqlx::query("INSERT INTO sessions (user_id, token, expires_at) VALUES (?1, ?2, ?3)")
        .bind(user_id)
        .bind(&token)
        .bind(&expires)
        .execute(&pool)
        .await
        .unwrap();
    token
}

/// Seed: one user, one book, one unsettled record, one settled record.
/// Returns (user_id, token, book_id, unsettled_id, settled_id).
async fn seed(db: &DbState) -> (i64, String, i64, i64, i64) {
    let pool = db.raw_pool().await;

    auth::do_init_admin(db, "testuser", "pass1234").await.unwrap();

    let user_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'testuser'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let token = create_session(db, user_id).await;

    let book_id: i64 =
        sqlx::query_scalar("INSERT INTO account_books (name) VALUES ('TestBook') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let unsettled_id: i64 = sqlx::query_scalar(
        "INSERT INTO income_records (book_id, date, category, description, total_amount, settlement_status) \
         VALUES (?1, '2024-01-01', 'Print', '打印订单', 50000, 'unsettled') RETURNING id",
    )
    .bind(book_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let settled_id: i64 = sqlx::query_scalar(
        "INSERT INTO income_records (book_id, date, category, description, total_amount, settlement_status, payment_date, payment_method) \
         VALUES (?1, '2024-01-02', 'Design', '设计费', 80000, 'settled', '2024-01-15', '银行转账') RETURNING id",
    )
    .bind(book_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    (user_id, token, book_id, unsettled_id, settled_id)
}

// ===== Settled records cannot be edited (via command) =====

#[tokio::test]
async fn cmd_settled_record_rejects_update() {
    let db = setup_db().await;
    let (_uid, _token, _bid, _unsettled, settled) = seed(&db).await;

    let err = record::do_update_record(
        &db,
        settled,
        "2024-06-01".into(),
        "Print".into(),
        None, None, None, None, None,
        99999,
        None,
    )
    .await
    .unwrap_err();

    assert!(err.contains("已结清"), "Expected '已结清' error, got: {}", err);
}

// ===== Settled records cannot be deleted (via command) =====

#[tokio::test]
async fn cmd_settled_record_rejects_delete() {
    let db = setup_db().await;
    let (_uid, _token, _bid, _unsettled, settled) = seed(&db).await;

    let err = record::do_delete_record(&db, settled).await.unwrap_err();
    assert!(err.contains("已结清"), "Expected '已结清' error, got: {}", err);
}

// ===== Unsettled record CAN be edited and deleted =====

#[tokio::test]
async fn cmd_unsettled_record_allows_update_and_delete() {
    let db = setup_db().await;
    let (_uid, _token, _bid, unsettled, _settled) = seed(&db).await;

    record::do_update_record(
        &db,
        unsettled,
        "2024-06-01".into(),
        "Copy".into(),
        Some("更新描述".into()),
        None, None, None, None,
        60000,
        None,
    )
    .await
    .unwrap();

    record::do_delete_record(&db, unsettled).await.unwrap();

    let pool = db.raw_pool().await;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM income_records WHERE id = ?1")
        .bind(unsettled)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

// ===== Export rejects settled records (via command) =====

#[tokio::test]
async fn cmd_export_rejects_settled_records() {
    let db = setup_db().await;
    let (_uid, _token, bid, unsettled, settled) = seed(&db).await;

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();

    let err = export::do_export_excel(&db, bid, vec![unsettled, settled], &path)
        .await
        .unwrap_err();

    assert!(err.contains("只能导出未结清"), "Expected settled rejection, got: {}", err);
}

// ===== Export with only unsettled records succeeds =====

#[tokio::test]
async fn cmd_export_allows_unsettled_records() {
    let db = setup_db().await;
    let (_uid, _token, bid, unsettled, _settled) = seed(&db).await;

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().with_extension("xlsx").to_string_lossy().to_string();

    let result = export::do_export_excel(&db, bid, vec![unsettled], &path)
        .await
        .unwrap();

    assert_eq!(result, path);
    assert!(std::path::Path::new(&path).exists());
}

// ===== export_all_unsettled via command =====

#[tokio::test]
async fn cmd_export_all_unsettled_works() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().with_extension("xlsx").to_string_lossy().to_string();

    let result = export::do_export_all_unsettled(&db, bid, &path)
        .await
        .unwrap();

    assert_eq!(result, path);
}

// ===== delete_user cannot delete self (via command) =====

#[tokio::test]
async fn cmd_delete_user_rejects_self() {
    let db = setup_db().await;
    let (uid, _token, _bid, _unsettled, _settled) = seed(&db).await;

    let err = auth::do_delete_user(&db, uid, uid).await.unwrap_err();
    assert!(err.contains("不能删除"), "Expected self-deletion error, got: {}", err);
}

// ===== delete_user CAN delete another user (via command) =====

#[tokio::test]
async fn cmd_delete_user_allows_other() {
    let db = setup_db().await;
    let (uid, _token, _bid, _unsettled, _settled) = seed(&db).await;

    auth::do_create_user(&db, "other", "pass5678").await.unwrap();

    let pool = db.raw_pool().await;
    let other_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'other'")
        .fetch_one(&pool)
        .await
        .unwrap();

    auth::do_delete_user(&db, other_id, uid).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = ?1")
        .bind(other_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

// ===== list_records: filter by category (via command) =====

#[tokio::test]
async fn cmd_list_records_filter_category() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;

    let res = record::do_list_records(
        &db, bid,
        Some("Print".into()), None, None, None, None, None, None,
    )
    .await
    .unwrap();

    assert_eq!(res.total, 1);
    assert_eq!(res.records[0].category, "Print");
}

// ===== list_records: filter by settlement_status (via command) =====

#[tokio::test]
async fn cmd_list_records_filter_status() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;

    let res = record::do_list_records(
        &db, bid,
        None, Some("unsettled".into()), None, None, None, None, None,
    )
    .await
    .unwrap();

    assert_eq!(res.total, 1);
    assert_eq!(res.records[0].settlement_status, "unsettled");
}

// ===== list_records: filter by date range (via command) =====

#[tokio::test]
async fn cmd_list_records_filter_date_range() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;

    let res = record::do_list_records(
        &db, bid,
        None, None,
        Some("2024-01-01".into()),
        Some("2024-01-31".into()),
        None, None, None,
    )
    .await
    .unwrap();
    assert_eq!(res.total, 2);

    let res = record::do_list_records(
        &db, bid,
        None, None,
        Some("2024-01-02".into()),
        Some("2024-01-02".into()),
        None, None, None,
    )
    .await
    .unwrap();
    assert_eq!(res.total, 1);
    assert_eq!(res.records[0].date, "2024-01-02");
}

// ===== list_records: keyword filter (via command) =====

#[tokio::test]
async fn cmd_list_records_filter_keyword() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;

    let res = record::do_list_records(
        &db, bid,
        None, None, None, None,
        Some("打印".into()),
        None, None,
    )
    .await
    .unwrap();
    assert_eq!(res.total, 1);
    assert!(res.records[0].description.contains("打印"));

    let res = record::do_list_records(
        &db, bid,
        None, None, None, None,
        Some("设计".into()),
        None, None,
    )
    .await
    .unwrap();
    assert_eq!(res.total, 1);
    assert!(res.records[0].description.contains("设计"));
}

// ===== list_records: pagination (via command) =====

#[tokio::test]
async fn cmd_list_records_pagination() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;
    let pool = db.raw_pool().await;

    for i in 3..11 {
        sqlx::query(
            "INSERT INTO income_records (book_id, date, category, total_amount) VALUES (?1, ?2, 'Other', ?3)",
        )
        .bind(bid)
        .bind(format!("2024-03-{:02}", i))
        .bind(i * 10000)
        .execute(&pool)
        .await
        .unwrap();
    }

    let p1 = record::do_list_records(
        &db, bid,
        None, None, None, None, None,
        Some(1), Some(3),
    )
    .await
    .unwrap();
    assert_eq!(p1.total, 10);
    assert_eq!(p1.records.len(), 3);

    let p2 = record::do_list_records(
        &db, bid,
        None, None, None, None, None,
        Some(2), Some(3),
    )
    .await
    .unwrap();
    assert_eq!(p2.records.len(), 3);

    let p1_ids: Vec<i64> = p1.records.iter().map(|r| r.id).collect();
    let p2_ids: Vec<i64> = p2.records.iter().map(|r| r.id).collect();
    for id in &p1_ids {
        assert!(!p2_ids.contains(id), "Pages must not overlap");
    }

    let p4 = record::do_list_records(
        &db, bid,
        None, None, None, None, None,
        Some(4), Some(3),
    )
    .await
    .unwrap();
    assert_eq!(p4.records.len(), 1);
}

// ===== list_records: returns images (via command) =====

#[tokio::test]
async fn cmd_list_records_includes_images() {
    let db = setup_db().await;
    let (_uid, _token, bid, unsettled, _settled) = seed(&db).await;
    let pool = db.raw_pool().await;

    sqlx::query(
        "INSERT INTO income_images (record_id, file_path, original_name) VALUES (?1, 'images/test.jpg', 'test.jpg')",
    )
    .bind(unsettled)
    .execute(&pool)
    .await
    .unwrap();

    let res = record::do_list_records(
        &db, bid,
        None, Some("unsettled".into()), None, None, None, None, None,
    )
    .await
    .unwrap();

    assert_eq!(res.records.len(), 1);
    assert_eq!(res.records[0].images.len(), 1);
    assert_eq!(res.records[0].images[0].original_name, "test.jpg");
}

// ===== list_records: no keyword → no bind mismatch (regression) =====

#[tokio::test]
async fn cmd_list_records_no_keyword_works() {
    let db = setup_db().await;
    let (_uid, _token, bid, _unsettled, _settled) = seed(&db).await;

    let res = record::do_list_records(&db, bid, None, None, None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(res.total, 2);
}

// ===== Database integrity check =====

#[tokio::test]
async fn cmd_integrity_check_passes() {
    let db = setup_db().await;
    let result = db.check_integrity().await;
    assert!(result.is_none(), "Fresh DB should pass integrity check");
}
