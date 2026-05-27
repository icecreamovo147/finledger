# SDD: FinLedger — 广告公司记账软件

## 1. Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                    Tauri 2 Desktop App                    │
│                                                          │
│  ┌─────────────────────┐   ┌──────────────────────────┐  │
│  │    Vue 3 Frontend    │   │     Rust Backend (Core)  │  │
│  │                     │   │                          │  │
│  │  Element Plus UI    │   │  Tauri Commands          │  │
│  │  Vxe Table          │   │  ┌─────────────────┐    │  │
│  │  ECharts (vue-echarts)│  │  │  Auth Module     │    │  │
│  │  Pinia Stores       │───│  │  Book Module     │    │  │
│  │  Vue Router          │   │  │  Record Module   │    │  │
│  │                     │   │  │  Export Module   │    │  │
│  │  invoke("command")  │   │  │  Backup Module   │    │  │
│  └─────────────────────┘   │  │  Dashboard Module│    │  │
│                            │  └─────────────────┘    │  │
│                            │         │               │  │
│                            │      sqlx (async)       │  │
│                            │         │               │  │
│                            │  ┌──────▼──────┐        │  │
│                            │  │   SQLite     │        │  │
│                            │  │  (local .db) │        │  │
│                            │  └─────────────┘        │  │
│                            └──────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

### Module Breakdown

| 模块 | 层级 | 职责 |
|------|------|------|
| **Auth Module** | Rust | 登录验证、密码哈希、会话管理、用户 CRUD |
| **Book Module** | Rust | 账本 CRUD |
| **Record Module** | Rust | 收入记录 CRUD、图片管理、结算状态流转 |
| **Export Module** | Rust | 生成 Excel 文件（rust_xlsxwriter） |
| **Backup Module** | Rust | 数据库文件复制/还原、文件系统操作 |
| **Dashboard Module** | Rust | 聚合查询（本月收入、未结清汇总、账本排名） |
| **UI Layer** | Vue 3 | 登录页、首页看板、账本管理、记录表格、表单弹窗、用户管理、备份恢复 |

### Communication Flow

```
[Vue Component] → [Pinia Action] → [invoke("command", args)]
                                         │
                                    [Tauri Command]
                                         │
                                    [Rust Handler]
                                         │
                                    [sqlx query]
                                         │
                                    [SQLite]
                                         │
                                    [Result<T, E>]
                                         │
[Vue Component] ← [Pinia State] ← [serde JSON deserialize]
```

---

## 2. Technology Choices & Dependencies

### Frontend (Vue 3)

| 依赖 | 版本 | 用途 |
|------|------|------|
| vue | ^3.5 | 核心框架 |
| vue-router | ^4.x | 路由管理（含登录守卫） |
| pinia | ^2.x | 状态管理 |
| element-plus | ^2.x | UI 组件库（表单、表格、弹窗、消息提示） |
| vxe-table | latest | 高性能表格（记录列表，支持复选框勾选） |
| vxe-pc-ui | latest | Vxe Table 配套 UI |
| echarts | ^5.x | 图表渲染 |
| vue-echarts | ^7.x | ECharts Vue3 组件封装 |
| @tauri-apps/api | ^2.x | Tauri invoke / 文件对话框等 API 调用 |

### Backend (Rust)

| 依赖（crate） | 用途 |
|---------------|------|
| tauri | Tauri 2 核心框架 |
| sqlx (runtime-tokio + sqlite) | 异步 SQLite 操作 + 编译期 SQL 校验 |
| serde / serde_json | 序列化/反序列化 |
| bcrypt | 密码哈希 |
| uuid | 会话 token 生成 |
| rust_xlsxwriter | Excel 文件生成 |
| chrono | 时间日期处理 |

### Dev Tools

| 工具 | 用途 |
|------|------|
| Vite | 前端构建 |
| TypeScript | 类型安全 |
| ESLint + Prettier | 代码规范 |
| cargo | Rust 构建 |
| tauri-cli | Tauri 开发/构建 |

---

## 3. Data Model

### 3.1 SQLite Schema

```sql
-- =====================
-- 用户表
-- =====================
CREATE TABLE users (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    username    TEXT    NOT NULL UNIQUE,
    password_hash TEXT  NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- =====================
-- 会话表（记住我 功能）
-- =====================
CREATE TABLE sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL,
    token       TEXT    NOT NULL UNIQUE,
    expires_at  TEXT    NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- =====================
-- 账本表（每个客户公司一个账本）
-- =====================
CREATE TABLE account_books (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    remark      TEXT    DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- =====================
-- 收入记录表
-- =====================
CREATE TABLE income_records (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id           INTEGER NOT NULL,
    date              TEXT    NOT NULL,
    category          TEXT    NOT NULL,
    description       TEXT    DEFAULT '',
    quantity          INTEGER,
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

CREATE INDEX idx_income_records_book_id ON income_records(book_id);
CREATE INDEX idx_income_records_status ON income_records(settlement_status);
CREATE INDEX idx_income_records_date ON income_records(date);

-- =====================
-- 收入记录图片表
-- =====================
CREATE TABLE income_images (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    record_id     INTEGER NOT NULL,
    file_path     TEXT    NOT NULL,
    original_name TEXT    NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    FOREIGN KEY (record_id) REFERENCES income_records(id) ON DELETE CASCADE
);

CREATE INDEX idx_income_images_record_id ON income_images(record_id);
```

### 3.2 Rust Domain Types

```rust
// category 枚举
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
pub enum IncomeCategory {
    PrintCopy,        // 打印/复印
    BindingFinish,    // 装订/后加工
    Design,           // 广告设计费
    MaterialProd,     // 物料制作
    AdRental,         // 广告位租赁/代理投放
    Installation,     // 安装费
    Other,            // 其他
}

// settlement_status 枚举
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum SettlementStatus {
    Unsettled,
    Settled,
}
```

### 3.3 TypeScript Types (Frontend)

```typescript
// 收入类别枚举（中文显示名）
type IncomeCategory =
  | 'PrintCopy'
  | 'BindingFinish'
  | 'Design'
  | 'MaterialProd'
  | 'AdRental'
  | 'Installation'
  | 'Other';

interface User {
  id: number;
  username: string;
  created_at: string;
}

interface AccountBook {
  id: number;
  name: string;
  remark: string;
  created_at: string;
  updated_at: string;
  // 聚合字段（查询时计算）
  total_unsettled?: number;
  record_count?: number;
}

interface IncomeRecord {
  id: number;
  book_id: number;
  date: string;
  category: IncomeCategory;
  description: string;
  quantity?: number;
  unit_price?: number;
  size_info: string;
  total_amount: number;
  settlement_status: 'unsettled' | 'settled';
  payment_date?: string;
  payment_method?: string;
  remark: string;
  images: IncomeImage[];
  created_at: string;
  updated_at: string;
}

interface IncomeImage {
  id: number;
  record_id: number;
  file_path: string;
  original_name: string;
  created_at: string;
}

interface DashboardStats {
  current_month_income: number;     // 本月收入总额
  total_unsettled: number;          // 未结清总额
  book_ranking: {                   // 各账本欠款排名
    book_id: number;
    book_name: string;
    unsettled_amount: number;
  }[];
}
```

---

## 4. API Design (Tauri Commands)

所有 Tauri Command 遵循统一响应格式：

```rust
// 成功: Result<T, String> — Tauri 侧返回 Ok(serde_json::to_value(data))
// 失败: Err(String) — 前端通过 try/catch 捕获
```

> **注意**：Tauri 2 的 `#[tauri::command]` 可以直接返回 `Result<T, String>`，前端 `invoke` 在失败时抛出异常。

### 4.1 Auth Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `init_admin` | `{ username: string, password: string }` | `void` | 初始化管理员账号（仅当 users 表为空时可用） |
| `login` | `{ username: string, password: string, remember: bool }` | `{ user: User, token: string }` | 验证用户名密码，remember=true 时创建 7 天有效 session |
| `logout` | `{ token: string }` | `void` | 销毁当前 session |
| `validate_session` | `{ token: string }` | `User` | 校验 token 是否有效 |
| `list_users` | — | `User[]` | 获取所有用户 |
| `create_user` | `{ username: string, password: string }` | `void` | 创建新用户 |
| `delete_user` | `{ user_id: number }` | `void` | 删除用户（不能删除自己） |
| `change_password` | `{ user_id: number, old_password: string, new_password: string }` | `void` | 修改密码（需验证旧密码） |

### 4.2 Book Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `create_book` | `{ name: string, remark?: string }` | `AccountBook` | 创建账本 |
| `list_books` | — | `AccountBook[]` | 列出所有账本，含未结清总额 |
| `update_book` | `{ id: number, name: string, remark?: string }` | `void` | 编辑账本 |
| `delete_book` | `{ id: number }` | `void` | 删除账本（级联删除所有记录和图片） |

### 4.3 Record Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `create_record` | `CreateRecordPayload` | `IncomeRecord` | 创建收入记录 |
| `list_records` | `{ book_id: number, filters?: RecordFilter }` | `IncomeRecord[]` | 列出记录（支持筛选） |
| `get_record` | `{ id: number }` | `IncomeRecord` | 获取单条记录详情（含图片） |
| `update_record` | `{ id: number, ...fields }` | `void` | 编辑记录（已结清记录拒绝修改） |
| `delete_record` | `{ id: number }` | `void` | 删除记录（已结清记录拒绝删除） |
| `settle_record` | `{ id: number, payment_date: string, payment_method: string }` | `void` | 标记结清 |
| `unsettle_record` | `{ id: number }` | `void` | 回退为未结清 |
| `upload_image` | `{ record_id: number, file_bytes: number[], file_name: string }` | `IncomeImage` | 上传图片 |
| `delete_image` | `{ id: number }` | `void` | 删除单张图片（同时删除文件） |

### 4.4 Export Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `export_excel` | `{ book_id: number, record_ids: number[], save_path: string }` | `string` | 导出 Excel，返回保存的文件路径 |

### 4.5 Dashboard Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `get_dashboard_stats` | — | `DashboardStats` | 获取首页看板统计数据 |

### 4.6 Backup Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `backup_database` | `{ target_dir: string }` | `string` | 备份数据库到目标目录，返回备份文件路径 |
| `restore_database` | `{ backup_path: string }` | `void` | 从备份文件恢复（覆盖当前数据库） |

### 4.7 CreateRecordPayload

```rust
struct CreateRecordPayload {
    book_id: i64,
    date: String,
    category: String,
    description: Option<String>,
    quantity: Option<i64>,
    unit_price: Option<f64>,
    size_info: Option<String>,
    total_amount: f64,
    remark: Option<String>,
    // 图片在创建记录后单独上传
}
```

---

## 5. Core Flows

### 5.1 启动流程

```
App Start
    │
    ▼
┌──────────────┐    no users    ┌──────────────────┐
│ 读取 users 表 │ ────────────► │ InitWizard 页面    │
└──────┬───────┘               │ 创建管理员账号     │
       │ has users             │ → 写入 users 表    │
       ▼                       └────────┬─────────┘
┌──────────────┐                        │
│ 检查本地 token │◄───────────────────────┘
└──────┬───────┘
       │
   ┌───┴───┐
   ▼       ▼
token有效  token无效/过期
   │       │
   ▼       ▼
Dashboard  Login 页面
```

### 5.2 登录流程

```
Login Page
    │
    ├─ 输入 username + password
    ├─ 勾选"记住我"（可选）
    │
    ▼
invoke("login", { username, password, remember })
    │
    ├─ Rust: 查询 users WHERE username = ?
    ├─ Rust: bcrypt::verify(password, password_hash)
    │   ├─ 失败 → 计数 +1，>=5次？→ 返回错误"账户已锁定15分钟"
    │   └─ 成功 → 生成 UUID token → 写入 sessions 表
    │             → remember? expire=7天 : expire=24小时
    │
    ▼
前端: 将 token 存入 localStorage
     → Pinia authStore.login(user)
     → router.push("/dashboard")
```

### 5.3 结算流程

```
BookDetail 页面（记录列表）
    │
    ├─ 勾选一条或多条"未结清"记录
    ├─ 点击"标记结清"
    │
    ▼
弹出 SettlementDialog
    ├─ 必填: 收款日期 (date picker)
    ├─ 必填: 收款方式 (select: 现金/银行转账/微信/支付宝 + 自定义)
    ├─ 确认
    │
    ▼
invoke("settle_record", { id, payment_date, payment_method })
    │
    ├─ Rust: 校验 settlement_status = 'unsettled'
    ├─ Rust: UPDATE income_records SET status='settled', payment_date=?, payment_method=?
    │
    ▼
前端: 刷新列表 → 该记录行编辑/删除按钮置灰
```

### 5.4 导出流程

```
BookDetail 页面
    │
    ├─ 筛选 settlement_status = 'unsettled'
    ├─ 全选 或 手动勾选记录
    ├─ 点击"导出账单"
    │
    ▼
invoke("export_excel", { book_id, record_ids, save_path })
    │
    ├─ Rust: 按 record_ids 查询 income_records
    ├─ Rust: 使用 rust_xlsxwriter 生成 Excel
    │   ├─ 列: 日期 | 类别 | 描述 | 数量 | 单价 | 尺寸 | 金额 | 备注
    │   ├─ 末行: 合计金额
    ├─ Rust: 写入文件
    │
    ▼
前端: 提示"导出成功" → 可选打开文件所在文件夹
```

### 5.5 图片存储流程

```
RecordForm 组件
    │
    ├─ 用户选择图片文件（多选）
    ├─ 前端: 校验单张 ≤ 20MB
    │
    ▼
对每个文件:
    invoke("upload_image", { record_id, file_bytes, file_name })
        │
        ├─ Rust: 生成唯一文件名 (UUID + 原扩展名)
        ├─ Rust: 写入 {app_data_dir}/images/{uuid}.{ext}
        ├─ Rust: INSERT INTO income_images (record_id, file_path, original_name)
        │
        ▼
    前端: 列表追加缩略图预览
```

---

## 6. Error Handling Strategy

### Rust 层

```
统一错误类型:
enum AppError {
    AuthError(String),       // 登录失败、密码错误等
    NotFound(String),        // 资源不存在
    Forbidden(String),       // 已结清不可修改
    ValidationError(String), // 必填项缺失、金额负数等
    IoError(String),         // 文件读写失败
    DbError(String),         // 数据库错误
}
```

所有 Tauri Command 返回 `Result<T, String>`，错误信息直接返回给前端展示。

### 前端层

- **Tauri invoke 错误**: 在 Pinia action 中以 try/catch 捕获，通过 `ElMessage.error(msg)` 展示
- **表单校验**: 前端 Element Plus 表单规则优先校验（减少无效请求），后端二次兜底
- **网络/进程错误**: Tauri 本身无网络，仅进程通信。异常场景很少，统一显示"操作失败"提示

### 关键校验规则

| 校验项 | 前端 | 后端 |
|--------|------|------|
| 用户名/密码非空 | ✓ | ✓ |
| 金额 ≥ 0 | ✓ | ✓ |
| 日期格式合法 | ✓ | ✓ |
| 已结清记录不可修改 | — | ✓（唯一安全门） |
| 图片 ≤ 20MB | ✓ | — |
| 删除自己 | — | ✓ |

---

## 7. Security Considerations

### 7.1 密码安全
- 密码使用 bcrypt (cost=12) 哈希存储，不可逆
- 前端传输密码时仅通过 Tauri IPC（本地进程通信），不经过网络
- 修改密码时必须验证旧密码

### 7.2 会话安全
- Session token 使用 UUID v4 随机生成
- Token 存储在 localStorage（本地受信环境，安全风险可控）
- "记住我"有效期 7 天，不勾选则 24 小时过期
- 过期 session 由后台定时清理

### 7.3 数据安全
- SQLite 使用参数化查询（sqlx 编译期校验），防止 SQL 注入
- 数据全量本地存储，无远程传输风险
- 备份文件为原始 SQLite 文件，恢复时直接覆盖

### 7.4 登录防暴力破解
- 连续 5 次登录失败后，锁定该账号 15 分钟
- 锁定计时器在内存中维护（应用重启后重置）

---

## 8. TBD — Technical Decisions Awaiting Confirmation

> **以下决策项需要用户确认后才能进入实施阶段。**

### TBD-1: 图片存储目录
图片存储在 Tauri 的 app data directory 下。
- macOS: `~/Library/Application Support/com.finledger.app/images/`
- Windows: `C:\Users\<user>\AppData\Roaming\com.finledger.app\images\`
- 方案：集中存储，数据库只存相对路径（如 `images/uuid.jpg`）

**请确认**：是否接受此默认路径？还是需要自定义图片存储位置？

### TBD-2: 登录失败锁定策略
连续 5 次失败 → 锁定 15 分钟。这是一个本地桌面应用，锁定时长和次数是否需要调整？

### TBD-3: Excel 导出时机
导出 Excel 时，前端先从 Tauri 的文件保存对话框获取用户选择的路径，再传给 Rust 写入。这个流程是否符合预期？

### TBD-4: "记住我"默认行为
关闭应用再打开，如果勾选了"记住我"，自动登录进首页；如果没勾选，需要重新输入密码。有效期的设定（7 天 / 24 小时）是否合理？

### TBD-5: 版本化管理
是否需要为账目记录做简单的版本化管理（比如记录每次修改的旧值），还是只需当前最新数据，不需要历史记录？（当前 SPEC 中操作历史在 Out of Scope）
