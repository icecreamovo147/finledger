# Task List: FinLedger

> 依赖关系：从上到下顺序执行，同批任务可并行

---

## Phase 1: 项目初始化

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T1 | 项目脚手架搭建 | 使用 `npm create tauri-app@latest` 初始化 Tauri 2 + Vue 3 + TS 项目，安装全部依赖（Element Plus、Vxe Table、ECharts、Pinia、Vue Router），配置 Vite | Medium | — |
| T2 | Rust 依赖与数据库初始化 | 添加 sqlx、bcrypt、uuid、rust_xlsxwriter、serde、chrono 等 crate，编写数据库迁移 SQL，实现 sqlx 连接池初始化与 `db/` 模块结构 | Medium | T1 |
| T3 | 路由与布局框架 | 配置 Vue Router（含登录守卫），搭建 MainLayout 布局（侧边栏导航 + 内容区），定义各页面骨架组件 | Simple | T1 |

---

## Phase 2: 认证系统

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T4 | 初始化向导 | 首次启动检测 users 表为空 → 跳转 InitWizard 页面 → 创建管理员账号 → 写入数据库 | Medium | T2, T3 |
| T5 | 用户登录 | Login 页面 UI + Pinia authStore + Rust `login` command（含 bcrypt 验证、session 创建、登录失败锁定） | Medium | T2, T3 |
| T6 | 登录态持久化 | "记住我"功能：localStorage token 存储、`validate_session` 自动登录、路由守卫拦截未登录请求、logout 清理 | Medium | T5 |
| T7 | 用户管理 | 用户列表页、创建用户弹窗、删除用户（含二次确认 + 不能删除自己）、修改密码弹窗 | Simple | T5 |

---

## Phase 3: 账本管理

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T8 | 账本 CRUD（Rust 后端） | `create_book`、`list_books`（含未结清总额聚合）、`update_book`、`delete_book`（级联删除记录+图片） | Medium | T2 |
| T9 | 账本管理 UI | 账本列表页（卡片/表格）、新增/编辑弹窗、删除二次确认、点击进入账本详情 | Medium | T8, T3 |

---

## Phase 4: 收入记录

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T10 | 记录 CRUD（Rust 后端） | `create_record`、`list_records`（含筛选）、`get_record`（含图片）、`update_record`（已结清拒绝）、`delete_record`（已结清拒绝） | Complex | T2 |
| T11 | 图片上传/删除（Rust 后端） | `upload_image`（接收 bytes → 保存到 app data dir → 写入 income_images 表）、`delete_image`（删除文件 + 数据库记录） | Medium | T10 |
| T12 | 记录列表 UI | Vxe Table 展示记录（分页、排序、筛选），操作栏（编辑/删除/标记结清按钮状态）、复选框勾选 | Complex | T9, T10 |
| T13 | 记录表单 UI | Element Plus 表单弹窗（新增/编辑模态），字段：日期选择器、类别下拉、描述、数量×单价、尺寸、总金额、备注、图片上传区（多图缩略图预览） | Complex | T11, T12 |

---

## Phase 5: 结算与导出

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T14 | 结算与回退（Rust 后端） | `settle_record`（更新 status + payment_date + payment_method）、`unsettle_record`（回退为 unsettled） | Medium | T10 |
| T15 | 结算 UI | SettlementDialog：日期选择器 + 收款方式下拉（预设 + 自定义） + 确认，操作后刷新列表状态 | Medium | T14, T12 |
| T16 | Excel 导出 | Rust `export_excel`（rust_xlsxwriter 生成 Excel）+ 前端触发（调文件保存对话框 → 传路径给 Rust） | Medium | T10 |
| T17 | 导出 UI | 勾选记录 → 点击"导出账单"按钮 → 选择路径 → 导出成功提示 | Simple | T16, T12 |

---

## Phase 6: 首页看板

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T18 | 看板统计（Rust 后端） | `get_dashboard_stats`：SQL 聚合查询（本月收入 SUM、未结清 SUM、各账本未结清排名） | Simple | T8, T10 |
| T19 | 看板 UI | Dashboard 页面：4 个数字指标卡片（本月收入、未结清总额、记录总数、待结算数）+ 账本欠款排名柱状图（ECharts） | Medium | T18, T3 |

---

## Phase 7: 数据备份

| ID | Task | Description | Complexity | Depends |
|----|------|-------------|------------|---------|
| T20 | 备份与恢复（Rust 后端） | `backup_database`（复制 .db 文件到目标目录）、`restore_database`（覆盖当前数据库，二次确认） | Medium | T2 |
| T21 | 备份与恢复 UI | 设置页面：备份按钮（调文件夹选择对话框） + 恢复按钮（选文件 + 确认覆盖警告） | Simple | T20, T3 |

---

## 任务统计

| 指标 | 数值 |
|------|------|
| 总任务数 | 21 |
| Simple | 6 |
| Medium | 12 |
| Complex | 3 |
| 跨 Phase 并行 | Phase 2-7 中相同依赖基线的任务可并行（如 T8 和 T10 都只依赖 T2，可同时做） |
