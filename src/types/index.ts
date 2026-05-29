// 收入类别
export type IncomeCategory =
  | "Print"
  | "Copy"
  | "Binding"
  | "PostProcess"
  | "Design"
  | "MaterialProd"
  | "AdRental"
  | "AdAgency"
  | "Installation"
  | "Other";

export const IncomeCategoryLabels: Record<IncomeCategory, string> = {
  Print: "打印",
  Copy: "复印",
  Binding: "装订",
  PostProcess: "后加工",
  Design: "广告设计费",
  MaterialProd: "物料制作",
  AdRental: "广告位租赁",
  AdAgency: "代理投放",
  Installation: "安装费",
  Other: "其他",
};

// 结算状态
export type SettlementStatus = "unsettled" | "settled";

// 收款方式预设
export const PaymentMethods = ["现金", "银行转账", "微信", "支付宝"] as const;

// 用户
export interface User {
  id: number;
  username: string;
  created_at: string;
}

// 账本
export interface AccountBook {
  id: number;
  name: string;
  remark: string;
  created_at: string;
  updated_at: string;
  total_unsettled?: number;
  record_count?: number;
}

// 收入图片
export interface IncomeImage {
  id: number;
  record_id: number;
  file_path: string;
  original_name: string;
  created_at: string;
}

// 收入记录
export interface IncomeRecord {
  id: number;
  book_id: number;
  date: string;
  category: IncomeCategory;
  description: string;
  quantity?: number;
  unit: string;
  unit_price?: number;
  size_info: string;
  total_amount: number;
  settlement_status: SettlementStatus;
  payment_date?: string;
  payment_method?: string;
  remark: string;
  images: IncomeImage[];
  created_at: string;
  updated_at: string;
}

// 分页结果
export interface PaginatedBooks {
  total: number;
  books: AccountBook[];
}

export interface PaginatedRecords {
  total: number;
  total_unsettled: number;
  book_total_unsettled: number;
  records: IncomeRecord[];
}

// 看板统计
export interface DashboardStats {
  current_month_income: number;
  total_unsettled: number;
  total_records: number;
  pending_settlement: number;
  book_ranking: {
    book_id: number;
    book_name: string;
    unsettled_amount: number;
  }[];
  income_trend: {
    month: string;
    total_amount: number;
  }[];
  settlement_trend: {
    month: string;
    settled_amount: number;
    unsettled_amount: number;
  }[];
  category_share: {
    category: string;
    amount: number;
  }[];
}

// 记录筛选条件
export interface RecordFilter {
  category?: IncomeCategory;
  settlement_status?: SettlementStatus;
  date_from?: string;
  date_to?: string;
  keyword?: string;
}

// 备份管理
export interface BackupSettings {
  enabled: boolean;
  target_dir: string | null;
  frequency: string;
  time_of_day: string;
  day_of_week: number | null;
  day_of_month: number | null;
  interval_minutes: number | null;
  retention_mode: string;
  retention_count: number;
  retention_days: number;
  retention_size_mb: number;
}

export interface BackupFileInfo {
  file_name: string;
  path: string;
  backup_type: "auto" | "manual" | "unknown";
  created_at: string | null;
  size_bytes: number;
  format_version: number | null;
  images_count: number | null;
  is_valid: boolean;
  validation_message: string | null;
}

export interface BackupRunState {
  last_run_at: string | null;
  last_success_at: string | null;
  last_status: string | null;
  last_message: string | null;
  last_backup_path: string | null;
  last_auto_run_at: string | null;
}

export interface BackupOverview {
  settings: BackupSettings;
  total_count: number;
  auto_count: number;
  manual_count: number;
  unknown_count: number;
  total_size_bytes: number;
  oldest_backup_at: string | null;
  latest_backup_at: string | null;
  last_run_state: BackupRunState;
  next_backup_at: string | null;
  backups: BackupFileInfo[];
}
