<p align="center">
  <img src="src/assets/finledger-logo.png" alt="FinLedger logo" width="120" />
</p>

# FinLedger

FinLedger 是一款面向广告、文印、物料制作等本地服务型业务的桌面端收入记账软件。它以客户公司为账本，帮助团队记录收入流水、管理未结清账款、保存图片凭证，并导出 Excel 账单用于客户核对和催款。

项目基于 Tauri 2 + Vue 3 + Rust + SQLite 构建，数据默认保存在本机，适合单机离线使用，并预留了未来扩展多人协作的基础数据结构。

## Features

- 本地桌面应用：支持 macOS 和 Windows 10/11。
- 用户与会话：首次启动初始化管理员，支持登录、记住我、用户管理和密码修改。
- 账本管理：按客户公司创建独立账本，查看每个账本的未结清金额和记录数量。
- 收入记录：支持日期、服务项目及内容、规格、数量、单位、单价、总金额、备注和图片凭证。
- 结算状态：记录可在未结清和已结清之间流转，已结清记录默认锁定，避免误改账目。
- 图片附件：图片保存到本地应用数据目录，数据库仅保存引用路径。
- Excel 导出：导出全部未结清记录或勾选的未结清记录，支持嵌入图片凭证。
- 首页看板：展示本月收入、未结清金额、待结算数量、账本欠款排行和趋势图。
- 数据备份：支持手动备份、自动备份、备份历史、恢复和备份保留策略。
- 数据安全：SQLite WAL、完整性检查、恢复回滚和附件一致性检查。

## Tech Stack

| Layer | Technology |
| --- | --- |
| Desktop shell | Tauri 2 |
| Frontend | Vue 3, TypeScript, Vite |
| UI | Element Plus, Vxe Table |
| State & routing | Pinia, Vue Router |
| Charts | ECharts, vue-echarts |
| Backend | Rust, Tokio |
| Database | SQLite via sqlx |
| Export | rust_xlsxwriter |
| Backup format | `.flbackup` zip archive with manifest and checksum |

## Requirements

- Node.js 18+ recommended
- npm
- Rust stable toolchain
- Tauri system dependencies for your platform

For Tauri platform setup, see the official Tauri prerequisites documentation:
https://tauri.app/start/prerequisites/

## Quick Start

Install dependencies:

```bash
npm install
```

Run the frontend development server:

```bash
npm run dev
```

Run the Tauri desktop app in development mode:

```bash
npm run tauri dev
```

On first launch, FinLedger will guide you through creating the initial administrator account.

## Common Scripts

```bash
# Type-check and build the web frontend
npm run build

# Preview the built frontend
npm run preview

# Run Tauri commands, such as dev/build
npm run tauri

# Run Rust tests
cd src-tauri
cargo test
```

Build a desktop package:

```bash
npm run tauri build
```

The generated installer or app bundle will be created under `src-tauri/target/release/bundle`.

## Project Structure

```text
.
├── src/                    # Vue frontend
│   ├── components/          # Shared layout and UI components
│   ├── router/              # Vue Router configuration
│   ├── stores/              # Pinia stores
│   ├── styles/              # Global styles and theme variables
│   ├── types/               # Shared TypeScript types
│   ├── utils/               # Frontend helpers
│   └── views/               # Page-level views
├── src-tauri/               # Tauri and Rust backend
│   ├── src/commands/        # Tauri command modules
│   ├── src/db/              # SQLite connection and migrations
│   ├── src/models/          # Serializable Rust models
│   ├── tests/               # Backend integration tests
│   └── tauri.conf.json      # Tauri configuration
├── docs/                    # Product and design documentation
├── public/                  # Static public assets
└── package.json
```

## Data Storage

FinLedger stores application data in the operating system's app data directory resolved by Tauri. The backend creates:

- `finledger.db`: local SQLite database
- `images/`: uploaded income record images
- `backup_settings.json`: automatic backup configuration
- `backup_run_state.json`: last backup status

Database schema migrations are managed in Rust at startup. Passwords are hashed with bcrypt, and database access uses parameterized sqlx queries.

## Backup And Restore

FinLedger supports both one-off manual backup and scheduled automatic backup.

- New backups use the `.flbackup` format.
- A backup contains the SQLite database, uploaded images, and a manifest.
- The manifest stores metadata, image count, backup type, app version, and database SHA-256 checksum.
- Restore operations validate the archive and roll back on failure where possible.
- Legacy `.db` backups can still be restored, but they do not contain images.

## Cross-Platform Notes

The codebase is designed to work on:

- macOS 12+ on Apple Silicon and Intel
- Windows 10/11 x64

Implementation details that help portability:

- File operations use Rust `Path` / `PathBuf` instead of hard-coded separators.
- User-facing save and directory selection uses Tauri dialog APIs.
- App data is resolved through Tauri rather than fixed platform paths.
- Backup filenames sanitize dynamic parts before writing files.
- Scheduled backup time handling avoids panics around local time edge cases such as daylight saving transitions.

For release confidence, run builds and tests on each target platform before publishing installers.

## Testing

Frontend build check:

```bash
npm run build
```

Backend tests:

```bash
cd src-tauri
cargo test
```

The current Rust test suite covers business rules, settlement locking, export restrictions, backup/restore safety, migration idempotency, maintenance guards, image consistency, and rollback behavior.

## Development Notes

- All authenticated frontend calls should go through `safeInvoke` so the current session token is included.
- Money values are stored as integer cents in the backend to avoid floating-point precision issues.
- Settled records are intentionally protected from edit and delete operations.
- Image files are written to disk while metadata lives in SQLite; keep both paths in sync when touching record logic.
- Backup and restore operations use a maintenance guard to avoid concurrent writes.

## Roadmap Ideas

- Windows and macOS CI workflows
- Signed release artifacts
- More granular user roles
- Audit logs for sensitive operations
- Optional LAN collaboration mode
- PDF invoice or statement export

## Contributing

Issues and pull requests are welcome. For substantial changes, please open an issue first to discuss the problem, expected behavior, and compatibility impact.

Before submitting a change, please run:

```bash
npm run build
cd src-tauri
cargo test
```

## License

No open-source license has been added yet. If you plan to publish this repository publicly, add a `LICENSE` file before accepting external contributions.
