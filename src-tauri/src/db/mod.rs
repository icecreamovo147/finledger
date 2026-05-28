use sqlx::sqlite::SqlitePool;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub const AUTH_REQUIRED: &str = "会话无效或已过期，请重新登录";
pub const MAINTENANCE_IN_PROGRESS: &str = "系统维护中，请稍后再试";

#[derive(Clone)]
pub struct DbState {
    pool: Arc<Mutex<SqlitePool>>,
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
            self.state.end_maintenance();
        }
    }
}

impl DbState {
    pub fn new(pool: SqlitePool, app_data_dir: PathBuf) -> Self {
        let images_dir = app_data_dir.join("images");
        Self {
            pool: Arc::new(Mutex::new(pool)),
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
        let result: Result<Vec<(String,)>, _> =
            sqlx::query_as("PRAGMA integrity_check")
                .fetch_all(&pool)
                .await;

        match result {
            Ok(rows) if rows.iter().all(|(v,)| v == "ok") => {
                let mut err = self.integrity_error.lock().await;
                *err = None;
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
        if self.maintenance.load(Ordering::Acquire) {
            return Err(MAINTENANCE_IN_PROGRESS.into());
        }
        Ok(self.pool.lock().await.clone())
    }

    pub fn begin_maintenance(&self) -> bool {
        self.maintenance.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }

    pub fn end_maintenance(&self) {
        self.maintenance.store(false, Ordering::Release);
    }

    pub async fn replace_pool(&self, new_pool: SqlitePool) {
        let mut pool = self.pool.lock().await;
        *pool = new_pool;
    }

    pub async fn raw_pool(&self) -> SqlitePool {
        self.pool.lock().await.clone()
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
            std::fs::copy(&db_path, &pre_migration_path).ok();
        }

        // Define versioned migrations
        let migrations: Vec<(i32, &str, &str)> = vec![
            (1, "create_users", r#"
                CREATE TABLE IF NOT EXISTS users (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    username    TEXT    NOT NULL UNIQUE,
                    password_hash TEXT  NOT NULL,
                    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
                )
            "#),
            (2, "create_sessions", r#"
                CREATE TABLE IF NOT EXISTS sessions (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id     INTEGER NOT NULL,
                    token       TEXT    NOT NULL UNIQUE,
                    expires_at  TEXT    NOT NULL,
                    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                )
            "#),
            (3, "create_account_books", r#"
                CREATE TABLE IF NOT EXISTS account_books (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    name        TEXT    NOT NULL,
                    remark      TEXT    DEFAULT '',
                    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
                )
            "#),
            (4, "create_income_records", r#"
                CREATE TABLE IF NOT EXISTS income_records (
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
            "#),
            (5, "create_income_images", r#"
                CREATE TABLE IF NOT EXISTS income_images (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    record_id     INTEGER NOT NULL,
                    file_path     TEXT    NOT NULL,
                    original_name TEXT    NOT NULL,
                    created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                    FOREIGN KEY (record_id) REFERENCES income_records(id) ON DELETE CASCADE
                )
            "#),
            (6, "add_unit_column",
                "ALTER TABLE income_records ADD COLUMN unit TEXT DEFAULT ''"
            ),
            (8, "create_indices", ""),  // handled specially below
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
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_income_records_date ON income_records(date)")
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
                continue;
            }

            let mut tx = pool.begin().await?;
            let result = sqlx::query(sql)
                .execute(&mut *tx)
                .await;
            match result {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string();
                    if !msg.contains("duplicate column name") {
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
        }

        // Version 7: REAL -> INTEGER money migration (special handling)
        if !applied_versions.contains(&7) {
            let col_type: Option<(String,)> = sqlx::query_as(
                "SELECT type FROM pragma_table_info('income_records') WHERE name = 'total_amount'",
            )
            .fetch_optional(&pool)
            .await?;

            let needs_money_migration = col_type.as_ref().map_or(false, |(t,)| t == "REAL");

            if needs_money_migration {
                let mut tx = pool.begin().await?;
                sqlx::query("ALTER TABLE income_records RENAME COLUMN unit_price TO unit_price_real")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("ALTER TABLE income_records RENAME COLUMN total_amount TO total_amount_real")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("ALTER TABLE income_records ADD COLUMN unit_price INTEGER")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("ALTER TABLE income_records ADD COLUMN total_amount INTEGER NOT NULL DEFAULT 0")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE income_records SET unit_price = ROUND(unit_price_real * 100) WHERE unit_price_real IS NOT NULL")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE income_records SET total_amount = ROUND(total_amount_real * 100)")
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
            } else {
                // Already INTEGER columns, just mark migration as done
                sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (7, 'convert_money_to_integer')")
                    .execute(&pool)
                    .await?;
            }
        }

        // Post-migration integrity check
        if let Some(err) = self.check_integrity().await {
            eprintln!("迁移后完整性检测异常: {}", err);
        }

        // Clean up pre-migration backup on success
        if pre_migration_path.exists() {
            std::fs::remove_file(&pre_migration_path).ok();
        }

        Ok(())
    }

    pub async fn needs_init(&self) -> Result<bool, sqlx::Error> {
        let pool = self.raw_pool().await;
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(&pool).await?;
        Ok(count.0 == 0)
    }

    pub async fn validate_token(&self, token: &str) -> Result<i64, String> {
        let pool = self.get_pool().await?;
        let row: Option<(i64, String)> = sqlx::query_as(
            "SELECT user_id, expires_at FROM sessions WHERE token = ?1",
        )
        .bind(token)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let (user_id, expires_at) = row.ok_or(AUTH_REQUIRED)?;

        let expires = chrono::NaiveDateTime::parse_from_str(&expires_at, "%Y-%m-%d %H:%M:%S")
            .map_err(|_| AUTH_REQUIRED)?;
        let now = chrono::Utc::now().naive_utc();

        if now > expires {
            sqlx::query("DELETE FROM sessions WHERE token = ?1")
                .bind(token)
                .execute(&pool)
                .await
                .ok();
            return Err(AUTH_REQUIRED.into());
        }

        Ok(user_id)
    }
}
