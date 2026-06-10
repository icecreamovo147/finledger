use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Acquire;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

pub fn sqlite_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
        .busy_timeout(std::time::Duration::from_secs(5))
}
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub const AUTH_REQUIRED: &str = "会话无效或已过期，请重新登录";
pub const MAINTENANCE_IN_PROGRESS: &str = "系统维护中，请稍后再试";

#[derive(Clone)]
pub struct DbState {
    pool: Arc<RwLock<SqlitePool>>,
    maintenance: Arc<AtomicBool>,
    integrity_error: Arc<Mutex<Option<String>>>,
    pub app_data_dir: PathBuf,
    pub images_dir: PathBuf,
}

pub struct MaintenanceGuard<'a> {
    state: &'a DbState,
    acquired: bool,
}

impl<'a> MaintenanceGuard<'a> {
    pub fn try_acquire(state: &'a DbState) -> Result<Self, String> {
        if state.begin_maintenance() {
            Ok(Self {
                state,
                acquired: true,
            })
        } else {
            Err(MAINTENANCE_IN_PROGRESS.into())
        }
    }

    pub fn disarm(&mut self) {
        self.acquired = false;
    }
}

impl<'a> Drop for MaintenanceGuard<'a> {
    fn drop(&mut self) {
        if self.acquired {
            info!("释放维护锁");
            self.state.end_maintenance();
        }
    }
}

impl DbState {
    pub fn new(pool: SqlitePool, app_data_dir: PathBuf) -> Self {
        let raw_dir = app_data_dir.join("images");
        // Canonicalize to resolve symlinks (e.g. macOS /var → /private/var).
        // If the directory doesn't exist yet, canonicalize fails — fall back to
        // the raw path; downstream boundary checks canonicalize both sides.
        let images_dir = raw_dir.canonicalize().unwrap_or(raw_dir);
        Self {
            pool: Arc::new(RwLock::new(pool)),
            maintenance: Arc::new(AtomicBool::new(false)),
            integrity_error: Arc::new(Mutex::new(None)),
            images_dir,
            app_data_dir,
        }
    }

    pub fn maintenance_guard(&self) -> Result<MaintenanceGuard<'_>, String> {
        MaintenanceGuard::try_acquire(self)
    }

    pub async fn check_integrity(&self) -> Option<String> {
        let pool = self.raw_pool().await;
        let result: Result<Vec<(String,)>, _> = sqlx::query_as("PRAGMA integrity_check")
            .fetch_all(&pool)
            .await;

        match result {
            Ok(rows) if rows.iter().all(|(v,)| v == "ok") => {
                let mut err = self.integrity_error.lock().await;
                *err = None;
                info!("数据库完整性检测通过");
                None
            }
            Ok(rows) => {
                let msg: Vec<&str> = rows.iter().map(|(v,)| v.as_str()).collect();
                let error = format!("数据库完整性检测异常: {}", msg.join("; "));
                let mut err = self.integrity_error.lock().await;
                *err = Some(error.clone());
                Some(error)
            }
            Err(e) => {
                let error = format!("数据库完整性检测失败: {}", e);
                let mut err = self.integrity_error.lock().await;
                *err = Some(error.clone());
                Some(error)
            }
        }
    }

    pub async fn get_integrity_error(&self) -> Option<String> {
        self.integrity_error.lock().await.clone()
    }

    pub async fn get_pool(&self) -> Result<SqlitePool, String> {
        let pool = self.pool.read().await;
        if self.maintenance.load(Ordering::Acquire) {
            return Err(MAINTENANCE_IN_PROGRESS.into());
        }
        Ok(pool.clone())
    }

    pub fn begin_maintenance(&self) -> bool {
        self.maintenance
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn end_maintenance(&self) {
        self.maintenance.store(false, Ordering::Release);
    }

    pub async fn replace_pool(&self, new_pool: SqlitePool) {
        let mut pool = self.pool.write().await;
        *pool = new_pool;
    }

    pub async fn raw_pool(&self) -> SqlitePool {
        self.pool.read().await.clone()
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        let pool = self.raw_pool().await;

        // Create schema_migrations table first
        let mut tx = pool.begin().await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version     INTEGER PRIMARY KEY,
                name        TEXT    NOT NULL,
                applied_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        // Query applied versions
        let applied: Vec<(i32,)> = sqlx::query_as("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await?;
        let applied_versions: HashSet<i32> = applied.into_iter().map(|(v,)| v).collect();

        info!("已应用的迁移版本: {:?}", applied_versions);

        // Pre-migration backup for databases with existing data
        let db_path = self.app_data_dir.join("finledger.db");
        let pre_migration_path = self.app_data_dir.join("finledger.db.pre-migration");
        let has_existing_data = applied_versions.is_empty()
            && sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name != 'schema_migrations'")
                .fetch_one(&pool)
                .await?
                .0
                > 0;
        if has_existing_data && db_path.exists() {
            info!("检测到已有数据，创建迁移前备份: {}", pre_migration_path.display());
            std::fs::copy(&db_path, &pre_migration_path).ok();
        }

        // Define versioned migrations
        let migrations: Vec<(i32, &str, &str)> = vec![
            (
                1,
                "create_users",
                r#"
                CREATE TABLE IF NOT EXISTS users (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    username    TEXT    NOT NULL UNIQUE,
                    password_hash TEXT  NOT NULL,
                    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
                )
            "#,
            ),
            (
                2,
                "create_sessions",
                r#"
                CREATE TABLE IF NOT EXISTS sessions (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id     INTEGER NOT NULL,
                    token       TEXT    NOT NULL UNIQUE,
                    expires_at  TEXT    NOT NULL,
                    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                )
            "#,
            ),
            (
                3,
                "create_account_books",
                r#"
                CREATE TABLE IF NOT EXISTS account_books (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    name        TEXT    NOT NULL,
                    remark      TEXT    DEFAULT '',
                    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
                )
            "#,
            ),
            (
                4,
                "create_income_records",
                r#"
                CREATE TABLE IF NOT EXISTS income_records (
                    id                INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id           INTEGER NOT NULL,
                    date              TEXT    NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
                    service_content   TEXT    NOT NULL,
                    specification     TEXT    DEFAULT '',
                    quantity          INTEGER,
                    unit              TEXT    DEFAULT '',
                    unit_price        INTEGER,
                    total_amount      INTEGER NOT NULL,
                    settlement_status TEXT    NOT NULL DEFAULT 'unsettled' CHECK (settlement_status IN ('unsettled', 'settled')),
                    payment_date      TEXT,
                    payment_method    TEXT,
                    remark            TEXT    DEFAULT '',
                    created_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    updated_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    FOREIGN KEY (book_id) REFERENCES account_books(id) ON DELETE CASCADE
                )
            "#,
            ),
            (
                5,
                "create_income_images",
                r#"
                CREATE TABLE IF NOT EXISTS income_images (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    record_id     INTEGER NOT NULL,
                    file_path     TEXT    NOT NULL,
                    original_name TEXT    NOT NULL,
                    created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    FOREIGN KEY (record_id) REFERENCES income_records(id) ON DELETE CASCADE
                )
            "#,
            ),
            (8, "create_indices", ""),     // 6 (add_unit_column) merged into v4     // handled specially below
            (10, "unique_book_names", ""), // handled specially below
            (11, "add_check_constraints", ""), // handled specially below
        ];

        for (version, name, sql) in &migrations {
            if applied_versions.contains(version) {
                continue;
            }

            if *version == 8 {
                // Index creation is idempotent with IF NOT EXISTS, run in a transaction
                let mut tx = pool.begin().await?;
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_records_book_id ON income_records(book_id)")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_records_status ON income_records(settlement_status)")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_income_records_date ON income_records(date)",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_images_record_id ON income_images(record_id)")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)")
                    .bind(version)
                    .bind(name)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                info!("迁移 8 (create_indices) 已应用");
                continue;
            }

            if *version == 10 || *version == 11 {
                continue; // handled specially below
            }

            let mut tx = pool.begin().await?;
            let result = sqlx::query(sql).execute(&mut *tx).await;
            match result {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string();
                    // 仅对非 CREATE TABLE 语句容错 "duplicate column name"
                    // （如 ALTER TABLE ADD COLUMN 的幂等场景）。
                    // CREATE TABLE IF NOT EXISTS 本身已是幂等的，若报此错说明 SQL 有误。
                    let is_create_table = sql.trim_start().to_uppercase().starts_with("CREATE TABLE");
                    if is_create_table || !msg.contains("duplicate column name") {
                        return Err(e);
                    }
                    // duplicate column — idempotent, continue
                }
            }
            sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)")
                .bind(version)
                .bind(name)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            info!("迁移 {} ({}) 已应用", version, name);
        }

        // Version 7: REAL -> INTEGER money migration (special handling)
        if !applied_versions.contains(&7) {
            let col_type: Option<(String,)> = sqlx::query_as(
                "SELECT type FROM pragma_table_info('income_records') WHERE name = 'total_amount'",
            )
            .fetch_optional(&pool)
            .await?;

            let needs_money_migration = col_type.as_ref().is_some_and(|(t,)| t == "REAL");

            if needs_money_migration {
                info!("迁移 7: REAL → INTEGER 金额转换开始");
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "ALTER TABLE income_records RENAME COLUMN unit_price TO unit_price_real",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "ALTER TABLE income_records RENAME COLUMN total_amount TO total_amount_real",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query("ALTER TABLE income_records ADD COLUMN unit_price INTEGER")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "ALTER TABLE income_records ADD COLUMN total_amount INTEGER NOT NULL DEFAULT 0",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE income_records SET unit_price = ROUND(unit_price_real * 100) WHERE unit_price_real IS NOT NULL")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE income_records SET total_amount = ROUND(total_amount_real * 100)",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query("ALTER TABLE income_records DROP COLUMN unit_price_real")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("ALTER TABLE income_records DROP COLUMN total_amount_real")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (7, 'convert_money_to_integer')")
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                info!("迁移 7 (REAL→INTEGER 金额转换) 已应用");
            } else {
                // Already INTEGER columns, just mark migration as done
                sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (7, 'convert_money_to_integer')")
                    .execute(&pool)
                    .await?;
                info!("迁移 7 跳过: 列已是 INTEGER 类型");
            }
        }

        // Version 9: replace old income record fields with service fields.
        //
        // This handles local databases that already applied version 4 before the
        // service field refactor. New databases are already created with the
        // target schema above, but existing dev databases need a one-time table
        // rebuild because editing an already-applied migration has no effect.
        if !applied_versions.contains(&9) {
            let columns: Vec<(String,)> =
                sqlx::query_as("SELECT name FROM pragma_table_info('income_records')")
                    .fetch_all(&pool)
                    .await?;
            let column_names: HashSet<String> = columns.into_iter().map(|(name,)| name).collect();
            let has_service_fields =
                column_names.contains("service_content") && column_names.contains("specification");

            // Only rebuild if the new service fields don't exist yet.
            // Old columns (category, description, size_info) are harmless
            // leftovers — rebuilding when they're present would overwrite
            // existing service_content with description data.
            if !has_service_fields {
                info!("迁移 9: 表结构重构 (category/description → service_content/specification)");
                // Prevent other connections from accessing the database while
                // we rebuild the table with foreign_keys disabled.
                // If the guard is held by a concurrent backup, skip this run
                // and retry on next startup instead of crashing.
                if let Ok(_maintenance_guard) = self.maintenance_guard() {
                    let mut conn = pool.acquire().await?;
                    sqlx::query("PRAGMA foreign_keys = OFF")
                        .execute(&mut *conn)
                        .await?;
                    let migration_result: Result<(), sqlx::Error> = async {
                        let mut tx = conn.begin().await?;
                        sqlx::query(
                            r#"
                        CREATE TABLE income_records_new (
                            id                INTEGER PRIMARY KEY AUTOINCREMENT,
                            book_id           INTEGER NOT NULL,
                            date              TEXT    NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
                            service_content   TEXT    NOT NULL,
                            specification     TEXT    DEFAULT '',
                            quantity          INTEGER,
                            unit              TEXT    DEFAULT '',
                            unit_price        INTEGER,
                            total_amount      INTEGER NOT NULL,
                            settlement_status TEXT    NOT NULL DEFAULT 'unsettled' CHECK (settlement_status IN ('unsettled', 'settled')),
                            payment_date      TEXT,
                            payment_method    TEXT,
                            remark            TEXT    DEFAULT '',
                            created_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                            updated_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                            FOREIGN KEY (book_id) REFERENCES account_books(id) ON DELETE CASCADE
                        )
                        "#,
                        )
                        .execute(&mut *tx)
                        .await?;

                        sqlx::query(
                            r#"
                        INSERT INTO income_records_new (
                            id, book_id, date, service_content, specification, quantity, unit,
                            unit_price, total_amount, settlement_status, payment_date,
                            payment_method, remark, created_at, updated_at
                        )
                        SELECT
                            id,
                            book_id,
                            date,
                            COALESCE(NULLIF(TRIM(description), ''), '未填写服务项目') AS service_content,
                            COALESCE(size_info, '') AS specification,
                            quantity,
                            COALESCE(unit, '') AS unit,
                            unit_price,
                            total_amount,
                            settlement_status,
                            payment_date,
                            payment_method,
                            COALESCE(remark, '') AS remark,
                            created_at,
                            updated_at
                        FROM income_records
                        "#,
                        )
                        .execute(&mut *tx)
                        .await?;

                        sqlx::query("DROP TABLE income_records")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("ALTER TABLE income_records_new RENAME TO income_records")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_records_book_id ON income_records(book_id)")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_records_status ON income_records(settlement_status)")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_records_date ON income_records(date)")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_images_record_id ON income_images(record_id)")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (9, 'replace_record_service_fields')")
                            .execute(&mut *tx)
                            .await?;
                        tx.commit().await?;
                        Ok(())
                    }
                    .await;
                    sqlx::query("PRAGMA foreign_keys = ON")
                        .execute(&mut *conn)
                        .await?;
                    migration_result?;
                } else {
                    warn!("迁移 9: 无法获取维护锁，将在下次启动重试");
                }
            } else {
                sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (9, 'replace_record_service_fields')")
                    .execute(&pool)
                    .await?;
                info!("迁移 9 跳过: service_content/specification 列已存在");
            }
        }

        // Version 10: unique constraint on account_books.name
        if !applied_versions.contains(&10) {
            info!("迁移 10: 添加账本名称唯一约束");
            let mut tx = pool.begin().await?;

            // First, deduplicate existing books so the unique index can be created.
            // Find names that appear more than once, and rename older duplicates.
            let dup_rows: Vec<(String, i64)> = sqlx::query_as(
                "SELECT name, COUNT(*) AS cnt FROM account_books GROUP BY name HAVING cnt > 1",
            )
            .fetch_all(&mut *tx)
            .await?;

            for (dup_name, _dup_count) in &dup_rows {
                warn!("迁移 10: 发现重复账本名称 \"{}\"，将重命名旧记录", dup_name);
                let dup_records: Vec<(i64, String)> = sqlx::query_as(
                    "SELECT id, name FROM account_books WHERE name = ?1 ORDER BY id ASC",
                )
                .bind(dup_name)
                .fetch_all(&mut *tx)
                .await?;

                // Keep the oldest record (lowest id) as-is; rename the rest
                for (idx, (dup_id, _)) in dup_records.iter().enumerate().skip(1) {
                    let new_name = format!("{} (重复 {})", dup_name, idx);
                    sqlx::query("UPDATE account_books SET name = ?1 WHERE id = ?2")
                        .bind(&new_name)
                        .bind(dup_id)
                        .execute(&mut *tx)
                        .await?;
                }
            }

            sqlx::query(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_account_books_name ON account_books(name)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO schema_migrations (version, name) VALUES (10, 'unique_book_names')",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            info!("迁移 10 (unique_book_names) 已应用");
        }

        // Version 11: add CHECK constraints to income_records via triggers
        // (SQLite does not support ALTER TABLE ADD CHECK)
        if !applied_versions.contains(&11) {
            info!("迁移 11: 添加 CHECK 约束触发器");
            let mut tx = pool.begin().await?;
            // Enforce settlement_status IN ('unsettled', 'settled') via triggers
            sqlx::query(
                r#"CREATE TRIGGER IF NOT EXISTS income_records_check_settlement_insert
                BEFORE INSERT ON income_records
                WHEN NEW.settlement_status NOT IN ('unsettled', 'settled')
                BEGIN
                    SELECT RAISE(ABORT, 'settlement_status must be unsettled or settled');
                END"#,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"CREATE TRIGGER IF NOT EXISTS income_records_check_settlement_update
                BEFORE UPDATE ON income_records
                WHEN NEW.settlement_status NOT IN ('unsettled', 'settled')
                BEGIN
                    SELECT RAISE(ABORT, 'settlement_status must be unsettled or settled');
                END"#,
            )
            .execute(&mut *tx)
            .await?;
            // Enforce date format YYYY-MM-DD via triggers
            sqlx::query(
                r#"CREATE TRIGGER IF NOT EXISTS income_records_check_date_insert
                BEFORE INSERT ON income_records
                WHEN NEW.date NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                BEGIN
                    SELECT RAISE(ABORT, 'date must be in YYYY-MM-DD format');
                END"#,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"CREATE TRIGGER IF NOT EXISTS income_records_check_date_update
                BEFORE UPDATE ON income_records
                WHEN NEW.date NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                BEGIN
                    SELECT RAISE(ABORT, 'date must be in YYYY-MM-DD format');
                END"#,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO schema_migrations (version, name) VALUES (11, 'add_check_constraints')",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            info!("迁移 11 (add_check_constraints) 已应用");
        }

        // Post-migration integrity check — fail hard if corrupted
        if let Some(err) = self.check_integrity().await {
            error!("迁移后完整性检测异常，阻止应用继续运行: {}", err);
            return Err(sqlx::Error::Protocol(
                format!("数据库完整性检测异常，请从备份恢复: {}", err),
            ));
        }

        // Clean up pre-migration backup on success
        if pre_migration_path.exists() {
            std::fs::remove_file(&pre_migration_path).ok();
        }

        Ok(())
    }

    pub async fn needs_init(&self) -> Result<bool, sqlx::Error> {
        let pool = self.raw_pool().await;
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await?;
        Ok(count.0 == 0)
    }

    pub async fn validate_token(&self, token: &str) -> Result<i64, String> {
        let pool = self.get_pool().await?;
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT user_id, expires_at FROM sessions WHERE token = ?1")
                .bind(token)
                .fetch_optional(&pool)
                .await
                .map_err(|e| e.to_string())?;

        let (user_id, expires_at) = row.ok_or(AUTH_REQUIRED)?;

        let expires = chrono::NaiveDateTime::parse_from_str(&expires_at, "%Y-%m-%d %H:%M:%S")
            .map_err(|_| AUTH_REQUIRED)?;
        let now = chrono::Utc::now().naive_utc();

        if now > expires {
            if let Err(e) = sqlx::query("DELETE FROM sessions WHERE token = ?1")
                .bind(token)
                .execute(&pool)
                .await
            {
                warn!("清理过期会话失败 (token 已拒绝): {}", e);
            }
            return Err(AUTH_REQUIRED.into());
        }

        Ok(user_id)
    }
}
