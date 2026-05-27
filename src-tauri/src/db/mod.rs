use sqlx::sqlite::SqlitePool;
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

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                username    TEXT    NOT NULL UNIQUE,
                password_hash TEXT  NOT NULL,
                created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id     INTEGER NOT NULL,
                token       TEXT    NOT NULL UNIQUE,
                expires_at  TEXT    NOT NULL,
                created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS account_books (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT    NOT NULL,
                remark      TEXT    DEFAULT '',
                created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
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
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS income_images (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id     INTEGER NOT NULL,
                file_path     TEXT    NOT NULL,
                original_name TEXT    NOT NULL,
                created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
                FOREIGN KEY (record_id) REFERENCES income_records(id) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&pool)
        .await?;

        // Migration: add unit column for existing databases
        sqlx::query("ALTER TABLE income_records ADD COLUMN unit TEXT DEFAULT ''")
            .execute(&pool)
            .await
            .ok(); // ignore error if column already exists

        // Migration: convert REAL money columns to INTEGER (cents)
        // Check if total_amount is still REAL; only migrate once
        let col_type: Option<(String,)> = sqlx::query_as(
            "SELECT type FROM pragma_table_info('income_records') WHERE name = 'total_amount'",
        )
        .fetch_optional(&pool)
        .await?;

        let needs_money_migration = col_type.as_ref().map_or(false, |(t,)| t == "REAL");

        if needs_money_migration {
            // Step 1: rename old REAL columns
            sqlx::query("ALTER TABLE income_records RENAME COLUMN unit_price TO unit_price_real")
                .execute(&pool)
                .await
                .ok();
            sqlx::query("ALTER TABLE income_records RENAME COLUMN total_amount TO total_amount_real")
                .execute(&pool)
                .await
                .ok();
            // Step 2: add new INTEGER columns
            sqlx::query("ALTER TABLE income_records ADD COLUMN unit_price INTEGER")
                .execute(&pool)
                .await
                .ok();
            sqlx::query("ALTER TABLE income_records ADD COLUMN total_amount INTEGER NOT NULL DEFAULT 0")
                .execute(&pool)
                .await
                .ok();
            // Step 3: copy data with conversion (ROUND(real * 100))
            sqlx::query("UPDATE income_records SET unit_price = ROUND(unit_price_real * 100) WHERE unit_price_real IS NOT NULL")
                .execute(&pool)
                .await
                .ok();
            sqlx::query("UPDATE income_records SET total_amount = ROUND(total_amount_real * 100)")
                .execute(&pool)
                .await
                .ok();
            // Step 4: drop old REAL columns (SQLite >= 3.35.0)
            sqlx::query("ALTER TABLE income_records DROP COLUMN unit_price_real")
                .execute(&pool)
                .await
                .ok();
            sqlx::query("ALTER TABLE income_records DROP COLUMN total_amount_real")
                .execute(&pool)
                .await
                .ok();
        }

        // Create indices
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_records_book_id ON income_records(book_id);",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_records_status ON income_records(settlement_status);",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_records_date ON income_records(date);",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_images_record_id ON income_images(record_id);",
        )
        .execute(&pool)
        .await?;

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
