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
pub struct ImageUpload {
    pub file_bytes: Vec<u8>,
    pub file_name: String,
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
    pub category: String,
    pub description: String,
    pub quantity: Option<i64>,
    pub unit: String,
    pub unit_price: Option<i64>,
    pub size_info: String,
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
}
