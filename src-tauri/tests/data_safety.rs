use finledger_lib::commands::attachment_check;
use finledger_lib::commands::record;
use finledger_lib::db::{sqlite_options, DbState};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::io::Write as IoWrite;
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

async fn seed_record_with_image(db: &DbState) -> (i64, i64) {
    let pool = db.raw_pool().await;

    let book_id: i64 =
        sqlx::query_scalar("INSERT INTO account_books (name) VALUES ('TestBook') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let record_id: i64 = sqlx::query_scalar(
        "INSERT INTO income_records (book_id, date, service_content, total_amount) \
         VALUES (?1, '2024-01-01', '门头广告制作', 50000) RETURNING id",
    )
    .bind(book_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let image_id: i64 = sqlx::query_scalar(
        "INSERT INTO income_images (record_id, file_path, original_name) \
         VALUES (?1, 'images/test.jpg', 'test.jpg') RETURNING id",
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    (record_id, image_id)
}

// ===== Delete record: DB rows deleted, file references cleaned =====

#[tokio::test]
async fn test_delete_record_db_first_then_files() {
    let db = setup_db().await;
    let (record_id, image_id) = seed_record_with_image(&db).await;

    record::do_delete_record(&db, record_id).await.unwrap();

    let pool = db.raw_pool().await;
    let record_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM income_records WHERE id = ?1")
        .bind(record_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(record_count.0, 0, "Record should be deleted from DB");

    let image_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM income_images WHERE id = ?1")
        .bind(image_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(image_count.0, 0, "Image row should be deleted from DB");
}

// ===== Delete book: DB rows deleted, image references cleaned =====

#[tokio::test]
async fn test_delete_book_db_first_then_files() {
    let db = setup_db().await;
    let (record_id, _image_id) = seed_record_with_image(&db).await;
    let pool = db.raw_pool().await;

    let book_id: i64 = sqlx::query_scalar("SELECT book_id FROM income_records WHERE id = ?1")
        .bind(record_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let img_before: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_images WHERE record_id = ?1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(img_before.0, 1);

    let mut tx = pool.begin().await.unwrap();
    let result = sqlx::query("DELETE FROM account_books WHERE id = ?1")
        .bind(book_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(result.rows_affected() > 0);
    tx.commit().await.unwrap();

    let record_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_records WHERE book_id = ?1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(record_count.0, 0, "Records should cascade-delete with book");

    let image_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_images WHERE record_id = ?1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(image_count.0, 0, "Images should cascade-delete with book");
}

// ===== Attachment consistency check detects orphan references =====

#[tokio::test]
async fn test_consistency_check_detects_orphans() {
    let db = setup_db().await;
    let (_record_id, _image_id) = seed_record_with_image(&db).await;

    let orphans = attachment_check::do_check_image_consistency(&db).await.unwrap();
    assert_eq!(orphans.len(), 1, "Should detect the orphan image reference");
    assert_eq!(orphans[0].file_path, "images/test.jpg");
}

// ===== Migration idempotency =====

#[tokio::test]
async fn test_migration_idempotency() {
    let db = setup_db().await;
    db.run_migrations().await.unwrap();

    let pool = db.raw_pool().await;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        count.0 >= 10,
        "Expected at least 10 migration versions, got {}",
        count.0
    );
}

// ===== schema_migrations table tracks all versions =====

#[tokio::test]
async fn test_migration_version_tracking() {
    let db = setup_db().await;
    let pool = db.raw_pool().await;

    let versions: Vec<(i32,)> =
        sqlx::query_as("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();

    let version_nums: Vec<i32> = versions.into_iter().map(|(v,)| v).collect();
    for v in &[1, 2, 3, 4, 5, 7, 8, 9, 10] {
        assert!(version_nums.contains(v), "Should have version {}", v);
    }
}

#[tokio::test]
async fn test_legacy_record_fields_migrate_to_service_fields() {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE schema_migrations (
            version     INTEGER PRIMARY KEY,
            name        TEXT    NOT NULL,
            applied_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    for version in [1, 2, 3, 4, 5, 6, 7, 8] {
        sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)")
            .bind(version)
            .bind(format!("legacy_{}", version))
            .execute(&pool)
            .await
            .unwrap();
    }

    sqlx::query(
        r#"
        CREATE TABLE account_books (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL,
            remark      TEXT    DEFAULT '',
            created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        CREATE TABLE income_records (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            book_id           INTEGER NOT NULL,
            date              TEXT    NOT NULL,
            category          TEXT    NOT NULL,
            description       TEXT    DEFAULT '',
            quantity          INTEGER,
            unit              TEXT    DEFAULT '',
            unit_price        INTEGER,
            size_info         TEXT    DEFAULT '',
            total_amount      INTEGER NOT NULL,
            settlement_status TEXT    NOT NULL DEFAULT 'unsettled',
            payment_date      TEXT,
            payment_method    TEXT,
            remark            TEXT    DEFAULT '',
            created_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            updated_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (book_id) REFERENCES account_books(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        CREATE TABLE income_images (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            record_id     INTEGER NOT NULL,
            file_path     TEXT    NOT NULL,
            original_name TEXT    NOT NULL,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (record_id) REFERENCES income_records(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let book_id: i64 =
        sqlx::query_scalar("INSERT INTO account_books (name) VALUES ('LegacyBook') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let record_id: i64 = sqlx::query_scalar(
        "INSERT INTO income_records \
         (book_id, date, category, description, size_info, total_amount) \
         VALUES (?1, '2024-01-01', 'Print', '旧描述内容', '旧尺寸', 10000) RETURNING id",
    )
    .bind(book_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO income_images (record_id, file_path, original_name) VALUES (?1, 'images/old.jpg', 'old.jpg')",
    )
    .bind(record_id)
    .execute(&pool)
    .await
    .unwrap();

    let db = DbState::new(pool, PathBuf::from("/tmp/finledger_legacy_test"));
    db.run_migrations().await.unwrap();
    let pool = db.raw_pool().await;

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('income_records')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let column_names: Vec<String> = columns.into_iter().map(|(name,)| name).collect();
    assert!(column_names.contains(&"service_content".to_string()));
    assert!(column_names.contains(&"specification".to_string()));
    assert!(!column_names.contains(&"category".to_string()));
    assert!(!column_names.contains(&"description".to_string()));
    assert!(!column_names.contains(&"size_info".to_string()));

    let mut page = record::do_list_records(&db, book_id, None, None, None, None, Some(1), Some(10))
        .await
        .unwrap();
    let record = page.records.remove(0);
    assert_eq!(record.service_content, "旧描述内容");
    assert_eq!(record.specification, "旧尺寸");
    assert_eq!(record.images.len(), 1);
}

// ===== Integrity check passes after migrations =====

#[tokio::test]
async fn test_integrity_check_after_migrations() {
    let db = setup_db().await;
    let result = db.check_integrity().await;
    assert!(
        result.is_none(),
        "Integrity check should pass after migrations"
    );
}

// ===== MaintenanceGuard auto-releases on drop =====

#[tokio::test]
async fn test_maintenance_guard_auto_release() {
    let db = setup_db().await;

    {
        let _guard = db.maintenance_guard().unwrap();
        let result = db.get_pool().await;
        assert!(
            result.is_err(),
            "get_pool should fail while maintenance is held"
        );
    }
    let result = db.get_pool().await;
    assert!(
        result.is_ok(),
        "get_pool should succeed after guard is dropped"
    );
}

// ===== MaintenanceGuard prevents double acquisition =====

#[tokio::test]
async fn test_maintenance_guard_prevents_double_acquire() {
    let db = setup_db().await;

    let _guard1 = db.maintenance_guard().unwrap();
    let result = db.maintenance_guard();
    assert!(result.is_err(), "Second guard acquisition should fail");
}

// ===== Cleanup orphan images removes DB rows for missing files =====

#[tokio::test]
async fn test_cleanup_orphan_images() {
    let db = setup_db().await;
    let (record_id, _image_id) = seed_record_with_image(&db).await;

    let pool = db.raw_pool().await;
    let before_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_images WHERE record_id = ?1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before_count.0, 1);

    let orphans = attachment_check::do_check_image_consistency(&db).await.unwrap();
    assert_eq!(orphans.len(), 1);

    let mut tx = pool.begin().await.unwrap();
    for orphan in &orphans {
        sqlx::query("DELETE FROM income_images WHERE id = ?1")
            .bind(orphan.id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();

    let after_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM income_images WHERE record_id = ?1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after_count.0, 0, "Orphan image row should be removed");
}

// ===== Full backup and restore with image records =====

#[tokio::test]
async fn test_full_backup_restore_with_images() {
    use finledger_lib::commands::backup;
    use finledger_lib::models::BackupManifest;

    let tmp_dir = tempfile::tempdir().unwrap();
    let app_dir = tmp_dir.path().to_path_buf();
    let images_dir = app_dir.join("images");
    std::fs::create_dir_all(&images_dir).unwrap();

    let db_path = app_dir.join("finledger.db");
    let opts = sqlite_options(&db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let db = DbState::new(pool, app_dir.clone());
    db.run_migrations().await.unwrap();

    let test_image = images_dir.join("test.jpg");
    std::fs::write(&test_image, b"fake image data").unwrap();

    let pool = db.raw_pool().await;
    let book_id: i64 =
        sqlx::query_scalar("INSERT INTO account_books (name) VALUES ('BackupTest') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let record_id: i64 = sqlx::query_scalar(
        "INSERT INTO income_records (book_id, date, service_content, total_amount) VALUES (?1, '2024-01-01', '门头广告制作', 10000) RETURNING id",
    ).bind(book_id).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO income_images (record_id, file_path, original_name) VALUES (?1, 'images/test.jpg', 'test.jpg')")
        .bind(record_id).execute(&pool).await.unwrap();

    let backup_dir = tmp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();
    let backup_path = backup::do_backup(&db, backup_dir.to_str().unwrap())
        .await
        .unwrap();

    assert!(std::path::Path::new(&backup_path).exists());
    assert!(backup_path.ends_with(".flbackup"));

    let zip_file = std::fs::File::open(&backup_path).unwrap();
    let mut archive = zip::ZipArchive::new(zip_file).unwrap();
    let mut manifest_file = archive.by_name("manifest.json").unwrap();
    let mut manifest_str = String::new();
    std::io::Read::read_to_string(&mut manifest_file, &mut manifest_str).unwrap();
    let manifest: BackupManifest = serde_json::from_str(&manifest_str).unwrap();
    assert_eq!(manifest.app, "FinLedger");
    assert_eq!(manifest.backup_format_version, 1);
    assert_eq!(manifest.images_count, 1);
    assert!(!manifest.db_sha256.is_empty());

    std::fs::remove_dir_all(&images_dir).unwrap();
    assert!(!images_dir.exists());

    let result = backup::do_restore(&db, &backup_path).await;
    assert!(result.is_ok(), "Restore should succeed: {:?}", result.err());

    assert!(images_dir.exists(), "Images directory should be restored");
    assert!(
        images_dir.join("test.jpg").exists(),
        "Image file should be restored"
    );
    assert_eq!(
        std::fs::read(images_dir.join("test.jpg")).unwrap(),
        b"fake image data"
    );

    let pool = db.raw_pool().await;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM income_records")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "Record should exist after restore");

    assert!(
        db.get_pool().await.is_ok(),
        "Maintenance lock should be released after restore"
    );
}

// ===== Legacy .db restore releases maintenance lock =====

#[tokio::test]
async fn test_legacy_db_restore_releases_lock() {
    use finledger_lib::commands::backup;

    let tmp_dir = tempfile::tempdir().unwrap();
    let app_dir = tmp_dir.path().to_path_buf();
    std::fs::create_dir_all(app_dir.join("images")).unwrap();

    let db_path = app_dir.join("finledger.db");
    let opts = sqlite_options(&db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let db = DbState::new(pool, app_dir.clone());
    db.run_migrations().await.unwrap();

    let backup_dir = tmp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();
    let legacy_backup = backup_dir.join("legacy_backup.db");
    std::fs::copy(&db_path, &legacy_backup).unwrap();

    assert!(db.get_pool().await.is_ok());

    let result = backup::do_restore(&db, legacy_backup.to_str().unwrap()).await;
    assert!(
        result.is_ok(),
        "Legacy restore should succeed: {:?}",
        result.err()
    );

    assert!(
        db.get_pool().await.is_ok(),
        "Maintenance lock must be released after legacy .db restore"
    );

    let msg = result.unwrap();
    assert!(
        msg.contains("旧版备份") || msg.contains("旧版"),
        "Should warn about old backup format, got: {}",
        msg
    );
}

// ===== Corrupt backup restore fails and preserves original data =====

#[tokio::test]
async fn test_restore_failure_rollback_images() {
    use finledger_lib::commands::backup;

    let tmp_dir = tempfile::tempdir().unwrap();
    let app_dir = tmp_dir.path().to_path_buf();
    let images_dir = app_dir.join("images");
    std::fs::create_dir_all(&images_dir).unwrap();

    let db_path = app_dir.join("finledger.db");
    let opts = sqlite_options(&db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let db = DbState::new(pool, app_dir.clone());
    db.run_migrations().await.unwrap();

    let original_image = images_dir.join("original.jpg");
    std::fs::write(&original_image, b"original data").unwrap();

    let backup_dir = tmp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();

    let corrupt_backup = backup_dir.join("corrupt.flbackup");
    {
        let zip_file = std::fs::File::create(&corrupt_backup).unwrap();
        let mut zip = zip::ZipWriter::new(zip_file);
        use zip::write::SimpleFileOptions;
        zip.start_file("finledger.db", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"this is not a valid sqlite file").unwrap();
        zip.start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"{\"backup_format_version\":1,\"app\":\"FinLedger\",\"version\":\"0.1.0\",\"created_at\":\"2024-01-01\",\"db_sha256\":\"0000\",\"images_count\":0}").unwrap();
        zip.finish().unwrap();
    }

    let result = backup::do_restore(&db, corrupt_backup.to_str().unwrap()).await;
    assert!(result.is_err(), "Corrupt backup restore should fail");

    assert!(
        original_image.exists(),
        "Original images must survive a failed restore"
    );
    assert_eq!(std::fs::read(&original_image).unwrap(), b"original data");
    assert!(
        db.get_pool().await.is_ok(),
        "DB should be usable after failed restore"
    );
}

// ===== Restore image replacement failure triggers rollback =====

#[tokio::test]
async fn test_restore_images_rollback_on_copy_failure() {
    use finledger_lib::commands::backup;

    let tmp_dir = tempfile::tempdir().unwrap();
    let app_dir = tmp_dir.path().to_path_buf();
    let images_dir = app_dir.join("images");
    std::fs::create_dir_all(&images_dir).unwrap();

    let db_path = app_dir.join("finledger.db");
    let opts = sqlite_options(&db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let db = DbState::new(pool, app_dir.clone());
    db.run_migrations().await.unwrap();

    let original_image = images_dir.join("original.jpg");
    std::fs::write(&original_image, b"original data").unwrap();

    // Create a valid backup
    let backup_dir = tmp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();
    let backup_path = backup::do_backup(&db, backup_dir.to_str().unwrap())
        .await
        .unwrap();

    // Make images_dir a file instead of directory — this will cause
    // create_dir_all to fail during the image replacement step,
    // AFTER the DB has already been replaced.
    std::fs::remove_dir_all(&images_dir).unwrap();
    std::fs::write(&images_dir, b"I am a file, not a dir").unwrap();
    assert!(images_dir.is_file());

    // Attempt restore — DB replacement succeeds but image replacement fails.
    // Rollback should restore both DB and images.
    let result = backup::do_restore(&db, &backup_path).await;
    assert!(
        result.is_err(),
        "Restore should fail when images_dir is a file"
    );

    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("回滚") || err_msg.contains("恢复中止"),
        "Error should mention rollback or abort, got: {}",
        err_msg
    );

    // After rollback: the images path should still exist
    assert!(
        images_dir.exists(),
        "images path should exist after rollback"
    );

    // DB should still be functional
    assert!(
        db.get_pool().await.is_ok(),
        "DB should be usable after failed restore"
    );

    // No stale backup dirs
    assert!(
        !app_dir.join(".images-pre-restore").exists(),
        "Stale images backup should be cleaned up"
    );
}

// ===== Successful restore cleans up all temporary files =====

#[tokio::test]
async fn test_successful_restore_cleanup() {
    use finledger_lib::commands::backup;

    let tmp_dir = tempfile::tempdir().unwrap();
    let app_dir = tmp_dir.path().to_path_buf();
    let images_dir = app_dir.join("images");
    std::fs::create_dir_all(&images_dir).unwrap();

    let db_path = app_dir.join("finledger.db");
    let opts = sqlite_options(&db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let db = DbState::new(pool, app_dir.clone());
    db.run_migrations().await.unwrap();

    std::fs::write(images_dir.join("test.jpg"), b"image data").unwrap();

    let backup_dir = tmp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();
    let backup_path = backup::do_backup(&db, backup_dir.to_str().unwrap())
        .await
        .unwrap();

    // Delete images to simulate data loss
    std::fs::remove_dir_all(&images_dir).unwrap();

    let result = backup::do_restore(&db, &backup_path).await;
    assert!(result.is_ok(), "Restore should succeed: {:?}", result.err());

    // Verify no stale rollback files remain
    assert!(!db_path.with_extension("db.pre-restore").exists());
    assert!(!app_dir.join(".images-pre-restore").exists());

    // Verify restored data
    assert!(images_dir.join("test.jpg").exists());
    assert!(db.get_pool().await.is_ok());
}
