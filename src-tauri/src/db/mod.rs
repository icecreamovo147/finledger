use sqlx::sqlite::SqlitePool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct DbState {
    pub pool: SqlitePool,
    pub app_data_dir: PathBuf,
    pub images_dir: PathBuf,
}

impl DbState {
    pub fn new(pool: SqlitePool, app_data_dir: PathBuf) -> Self {
        let images_dir = app_data_dir.join("images");
        Self {
            pool,
            images_dir,
            app_data_dir,
        }
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
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
        .execute(&self.pool)
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
        .execute(&self.pool)
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
        .execute(&self.pool)
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
                unit_price        REAL,
                size_info         TEXT    DEFAULT '',
                total_amount      REAL    NOT NULL,
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
        .execute(&self.pool)
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
        .execute(&self.pool)
        .await?;

        // Migration: add unit column for existing databases
        sqlx::query("ALTER TABLE income_records ADD COLUMN unit TEXT DEFAULT ''")
            .execute(&self.pool)
            .await
            .ok(); // ignore error if column already exists

        // Create indices
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_records_book_id ON income_records(book_id);",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_records_status ON income_records(settlement_status);",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_records_date ON income_records(date);",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_income_images_record_id ON income_images(record_id);",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn needs_init(&self) -> Result<bool, sqlx::Error> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(&self.pool).await?;
        Ok(count.0 == 0)
    }
}
