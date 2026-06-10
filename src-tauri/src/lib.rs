pub mod commands;
pub mod db;
pub mod error;
pub mod logger;
pub mod models;
pub mod utils;

use commands::backup_scheduler::BackupSchedulerState;
use db::{sqlite_options, DbState};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tracing_appender::non_blocking::WorkerGuard;

/// 持有日志 WorkerGuard，确保退出时 flush 缓冲区
pub struct LogGuard(pub Option<WorkerGuard>);

#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState};
use tauri::AppHandle;
use tauri::Manager;
#[cfg(not(target_os = "macos"))]
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

pub struct LoginAttempts(Mutex<HashMap<String, (u32, chrono::DateTime<chrono::Utc>)>>);

impl LoginAttempts {
    /// 安全地获取锁，自动从中毒状态恢复。
    pub fn lock(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<String, (u32, chrono::DateTime<chrono::Utc>)>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!(target: "finledger", "LoginAttempts Mutex 已中毒，自动恢复");
                poisoned.into_inner()
            }
        }
    }
}
const TRAY_MENU_OPEN_ID: &str = "tray_open_main";
#[cfg(target_os = "macos")]
const TRAY_MENU_HIDE_ID: &str = "tray_hide_main";
const TRAY_MENU_EXIT_ID: &str = "tray_exit_app";

#[tauri::command]
fn exit_app(app: AppHandle) {
    // LogGuard 的 WorkerGuard drop 时会自动 flush 日志缓冲区
    app.exit(0);
}

fn show_main_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn hide_main_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(target_os = "macos")]
fn handle_tray_menu_event<R: tauri::Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        TRAY_MENU_OPEN_ID => show_main_window(app),
        TRAY_MENU_HIDE_ID => hide_main_window(app),
        TRAY_MENU_EXIT_ID => app.exit(0),
        _ => {}
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_tray_menu_event<R: tauri::Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        TRAY_MENU_OPEN_ID => show_main_window(app),
        TRAY_MENU_EXIT_ID => app.exit(0),
        _ => {}
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(LoginAttempts(Mutex::new(HashMap::new())))
        .manage(BackupSchedulerState::new())
        .manage(LogGuard(None))
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();
            std::fs::create_dir_all(app_dir.join("images")).ok();

            let guard = logger::init(&app_dir);
            app.manage(LogGuard(guard));
            tracing::info!(target: "finledger", "FinLedger 启动，数据目录: {}", app_dir.display());

            let db_path = app_dir.join("finledger.db");

            let pool = tauri::async_runtime::block_on(async {
                SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(sqlite_options(&db_path))
                    .await
                    .expect("failed to connect to database")
            });

            // Clean up stale staging sessions (older than 24h)
            commands::image_staging::cleanup_stale_staging_sessions(&app_dir);

            // Clean up orphan temp files from previous failed uploads
            let images_dir = app_dir.join("images");
            if let Ok(entries) = std::fs::read_dir(&images_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with(".tmp-") {
                        std::fs::remove_file(entry.path()).ok();
                    }
                }
            }

            // Clean up stale restore-tmp directories from failed restores
            if let Ok(entries) = std::fs::read_dir(&app_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with(".restore-tmp-") {
                        std::fs::remove_dir_all(entry.path()).ok();
                    }
                }
            }

            let db = DbState::new(pool, app_dir.clone());
            tauri::async_runtime::block_on(async {
                tracing::info!(target: "finledger", "开始数据库迁移...");
                // run_migrations 内部会在迁移后做完整性检查，异常时直接返回 error
                db.run_migrations().await.expect("failed to run migrations");
                tracing::info!(target: "finledger", "数据库迁移完成");
                // 二次确认完整性（run_migrations 已检查过，此处为防御性检查）
                if let Some(err) = db.check_integrity().await {
                    panic!("数据库完整性检测异常，应用无法启动: {}", err);
                }
            });

            // Start backup scheduler if configured
            let mut backup_settings = commands::backup_settings::get_backup_settings_sync(&db);
            // 首次启动：配置文件不存在时，自动创建默认备份目录并写入默认配置
            let settings_file = app_dir.join("backup_settings.json");
            if !settings_file.exists() {
                let default_backup_dir = app_dir.join("backups");
                if let Err(e) = std::fs::create_dir_all(&default_backup_dir) {
                    tracing::warn!(target: "finledger", "无法创建默认备份目录 {}: {}", default_backup_dir.display(), e);
                } else {
                    backup_settings.target_dir =
                        Some(default_backup_dir.to_string_lossy().to_string());
                    if let Err(e) = commands::backup_settings::save_settings(
                        &app_dir,
                        &backup_settings,
                    ) {
                        tracing::warn!(target: "finledger", "无法保存默认备份配置: {}", e);
                    } else {
                        tracing::info!(target: "finledger", "已创建默认备份配置: 每{}分钟自动备份到 {}",
                            backup_settings.interval_minutes.unwrap_or(30),
                            default_backup_dir.display());
                    }
                }
            }
            let db_for_scheduler = db.clone();
            let db_for_cleanup = db.clone();
            app.manage(db);
            let app_handle = app.handle().clone();
            // 启动时异步清理孤儿图片记录
            tauri::async_runtime::spawn(async move {
                match commands::attachment_check::do_check_image_consistency(&db_for_cleanup).await {
                    Ok(orphans) if !orphans.is_empty() => {
                        tracing::info!(target: "finledger", "启动时检测到 {} 个孤儿图片记录，开始清理", orphans.len());
                        let pool = match db_for_cleanup.get_pool().await {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::warn!(target: "finledger", "启动时清理孤儿图片失败(获取连接池): {}", e);
                                return;
                            }
                        };
                        if let Ok(mut tx) = pool.begin().await {
                            let mut cleaned = 0u64;
                            for orphan in &orphans {
                                if sqlx::query("DELETE FROM income_images WHERE id = ?1")
                                    .bind(orphan.id)
                                    .execute(&mut *tx)
                                    .await
                                    .is_ok()
                                {
                                    cleaned += 1;
                                }
                            }
                            if tx.commit().await.is_ok() {
                                tracing::info!(target: "finledger", "启动时清理孤儿图片完成: {} 条", cleaned);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(target: "finledger", "启动时检查图片一致性失败: {}", e);
                    }
                }
            });
            tauri::async_runtime::spawn(async move {
                let scheduler = app_handle.state::<BackupSchedulerState>();
                scheduler.restart(&db_for_scheduler, backup_settings).await;
                tracing::info!(target: "finledger", "备份调度器已启动");
            });

            #[cfg(target_os = "macos")]
            let icon = Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))
                .expect("failed to load macOS tray icon");
            #[cfg(not(target_os = "macos"))]
            let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
                .expect("failed to load tray icon");
            let open_item =
                MenuItem::with_id(app, TRAY_MENU_OPEN_ID, "打开主界面", true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let hide_item =
                MenuItem::with_id(app, TRAY_MENU_HIDE_ID, "隐藏主窗口", true, None::<&str>)?;
            let exit_item = MenuItem::with_id(app, TRAY_MENU_EXIT_ID, "退出", true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let tray_menu = Menu::with_items(app, &[&open_item, &hide_item, &exit_item])?;
            #[cfg(not(target_os = "macos"))]
            let tray_menu = Menu::with_items(app, &[&open_item, &exit_item])?;
            let _tray = TrayIconBuilder::new()
                .menu(&tray_menu)
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("FinLedger")
                .on_menu_event(handle_tray_menu_event)
                .on_tray_icon_event(|_tray_icon, _event| {
                    #[cfg(not(target_os = "macos"))]
                    {
                        if let tauri::tray::TrayIconEvent::Click {
                            button,
                            button_state,
                            ..
                        } = _event
                        {
                            if button == MouseButton::Left && button_state == MouseButtonState::Up {
                                show_main_window(&_tray_icon.app_handle());
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                #[cfg(target_os = "macos")]
                {
                    let _ = window.hide();
                }

                #[cfg(not(target_os = "macos"))]
                let window = window.clone();
                #[cfg(not(target_os = "macos"))]
                {
                    window
                        .dialog()
                        .message("请选择关闭方式")
                        .title("关闭 FinLedger")
                        .kind(MessageDialogKind::Info)
                        .buttons(MessageDialogButtons::YesNoCancelCustom(
                            "最小化到托盘".into(),
                            "直接关闭".into(),
                            "取消".into(),
                        ))
                        .parent(&window)
                        .show_with_result(move |result| match result {
                            MessageDialogResult::Custom(choice) if choice == "最小化到托盘" =>
                            {
                                let _ = window.set_skip_taskbar(true);
                                let _ = window.hide();
                            }
                            MessageDialogResult::Custom(choice) if choice == "直接关闭" => {
                                window.app_handle().exit(0);
                            }
                            _ => {}
                        });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::get_app_data_path,
            commands::auth::check_db_integrity,
            commands::auth::needs_init,
            commands::auth::init_admin,
            commands::auth::login,
            commands::auth::logout,
            commands::auth::validate_session,
            commands::auth::list_users,
            commands::auth::create_user,
            commands::auth::delete_user,
            commands::auth::change_password,
            commands::auth::admin_reset_password,
            commands::book::create_book,
            commands::book::list_books,
            commands::book::get_book,
            commands::book::update_book,
            commands::book::delete_book,
            commands::record::create_record,
            commands::record::list_records,
            commands::record::get_record,
            commands::record::update_record,
            commands::record::delete_record,
            commands::record::delete_image,
            commands::record::settle_record,
            commands::record::unsettle_record,
            commands::record::read_image_base64,
            commands::image_staging::stage_image_from_path,
            commands::image_staging::stage_image_bytes,
            commands::image_staging::delete_staged_image,
            commands::image_staging::cancel_image_staging_session,
            commands::image_staging::create_record_with_staged_images,
            commands::image_staging::update_record_with_staged_images,
            commands::attachment_check::check_attachment_consistency,
            commands::attachment_check::cleanup_orphan_images,
            commands::export::export_excel,
            commands::export::export_all_unsettled,
            commands::dashboard::get_dashboard_stats,
            commands::backup::backup_database,
            commands::backup::restore_database,
            commands::backup_settings::get_backup_settings,
            commands::backup_settings::update_backup_settings,
            commands::backup_settings::get_backup_overview,
            commands::backup_settings::run_backup_now,
            commands::backup_settings::restore_backup,
            commands::backup_settings::delete_backup_file,
            exit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = _event
            {
                if !has_visible_windows {
                    show_main_window(_app_handle);
                }
            }
        });
}
