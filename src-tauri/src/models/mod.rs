use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResult {
    pub user: User,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBook {
    pub id: i64,
    pub name: String,
    pub remark: String,
    pub created_at: String,
    pub updated_at: String,
    pub total_unsettled: Option<i64>,
    pub record_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeImage {
    pub id: i64,
    pub record_id: i64,
    pub file_path: String,
    pub original_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeRecord {
    pub id: i64,
    pub book_id: i64,
    pub date: String,
    pub service_content: String,
    pub specification: String,
    pub quantity: Option<i64>,
    pub unit: String,
    pub unit_price: Option<i64>,
    pub total_amount: i64,
    pub settlement_status: String,
    pub payment_date: Option<String>,
    pub payment_method: Option<String>,
    pub remark: String,
    pub images: Vec<IncomeImage>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookRanking {
    pub book_id: i64,
    pub book_name: String,
    pub unsettled_amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyIncome {
    pub month: String,
    pub total_amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySettlement {
    pub month: String,
    pub settled_amount: i64,
    pub unsettled_amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedBooks {
    pub total: i64,
    pub books: Vec<AccountBook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedRecords {
    pub total: i64,
    pub total_unsettled: i64,
    pub book_total_unsettled: i64,
    pub records: Vec<IncomeRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub current_month_income: i64,
    pub total_unsettled: i64,
    pub total_records: i64,
    pub pending_settlement: i64,
    pub book_ranking: Vec<BookRanking>,
    pub income_trend: Vec<MonthlyIncome>,
    pub settlement_trend: Vec<MonthlySettlement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub backup_format_version: u32,
    pub app: String,
    pub version: String,
    pub created_at: String,
    pub db_sha256: String,
    pub images_count: u32,
    #[serde(default)]
    pub backup_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSettings {
    pub enabled: bool,
    pub target_dir: Option<String>,
    pub frequency: String,
    pub time_of_day: String,
    #[serde(default)]
    pub day_of_week: Option<u32>,
    #[serde(default)]
    pub day_of_month: Option<u32>,
    #[serde(default)]
    pub interval_minutes: Option<u32>,
    pub retention_mode: String,
    pub retention_count: u32,
    pub retention_days: u32,
    pub retention_size_mb: u64,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            target_dir: None,
            frequency: "daily".into(),
            time_of_day: "23:00".into(),
            day_of_week: None,
            day_of_month: None,
            interval_minutes: None,
            retention_mode: "count".into(),
            retention_count: 10,
            retention_days: 30,
            retention_size_mb: 2048,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRunState {
    pub last_run_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_status: Option<String>,
    pub last_message: Option<String>,
    pub last_backup_path: Option<String>,
    #[serde(default)]
    pub last_auto_run_at: Option<String>,
}

impl Default for BackupRunState {
    fn default() -> Self {
        Self {
            last_run_at: None,
            last_success_at: None,
            last_status: None,
            last_message: None,
            last_backup_path: None,
            last_auto_run_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileInfo {
    pub file_name: String,
    pub path: String,
    pub backup_type: String,
    pub created_at: Option<String>,
    pub size_bytes: u64,
    pub format_version: Option<u32>,
    pub images_count: Option<u32>,
    pub is_valid: bool,
    pub validation_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupOverview {
    pub settings: BackupSettings,
    pub total_count: usize,
    pub auto_count: usize,
    pub manual_count: usize,
    pub unknown_count: usize,
    pub total_size_bytes: u64,
    pub oldest_backup_at: Option<String>,
    pub latest_backup_at: Option<String>,
    pub last_run_state: BackupRunState,
    pub next_backup_at: Option<String>,
    pub backups: Vec<BackupFileInfo>,
}
