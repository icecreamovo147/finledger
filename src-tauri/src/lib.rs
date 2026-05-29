pub mod commands;
pub mod db;
pub mod models;

use commands::backup_scheduler::BackupSchedulerState;
use db::{sqlite_options, DbState};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState};
use tauri::AppHandle;
use tauri::Manager;
#[cfg(not(target_os = "macos"))]
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

pub struct LoginAttempts(pub Mutex<HashMap<String, (u32, chrono::DateTime<chrono::Utc>)>>);
const TRAY_MENU_OPEN_ID: &str = "tray_open_main";
#[cfg(target_os = "macos")]
const TRAY_MENU_HIDE_ID: &str = "tray_hide_main";
const TRAY_MENU_EXIT_ID: &str = "tray_exit_app";

#[tauri::command]
fn exit_app(app: AppHandle) {
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
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();
            std::fs::create_dir_all(app_dir.join("images")).ok();

            let db_path = app_dir.join("finledger.db");

            let pool = tauri::async_runtime::block_on(async {
                SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(sqlite_options(&db_path))
                    .await
                    .expect("failed to connect to database")
            });

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

            let db = DbState::new(pool, app_dir);
            tauri::async_runtime::block_on(async {
                db.run_migrations().await.expect("failed to run migrations");
                if let Some(err) = db.check_integrity().await {
                    eprintln!("{}", err);
                }
            });

            // Start backup scheduler if configured
            let backup_settings = commands::backup_settings::get_backup_settings_sync(&db);
            let db_for_scheduler = db.clone();
            app.manage(db);
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let scheduler = app_handle.state::<BackupSchedulerState>();
                scheduler.restart(&db_for_scheduler, backup_settings).await;
            });

            let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
                .expect("failed to load tray icon");
            let open_item = MenuItem::with_id(app, TRAY_MENU_OPEN_ID, "打开主界面", true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let hide_item = MenuItem::with_id(app, TRAY_MENU_HIDE_ID, "隐藏主窗口", true, None::<&str>)?;
            let exit_item = MenuItem::with_id(app, TRAY_MENU_EXIT_ID, "退出", true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let tray_menu = Menu::with_items(app, &[&open_item, &hide_item, &exit_item])?;
            #[cfg(not(target_os = "macos"))]
            let tray_menu = Menu::with_items(app, &[&open_item, &exit_item])?;
            let _tray = TrayIconBuilder::new()
                .menu(&tray_menu)
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .icon(icon)
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
                            MessageDialogResult::Custom(choice) if choice == "最小化到托盘" => {
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
            commands::book::create_book,
            commands::book::list_books,
            commands::book::get_book,
            commands::book::update_book,
            commands::book::delete_book,
            commands::record::create_record,
            commands::record::create_record_with_images,
            commands::record::list_records,
            commands::record::get_record,
            commands::record::update_record,
            commands::record::update_record_with_images,
            commands::record::delete_record,
            commands::record::upload_image,
            commands::record::delete_image,
            commands::record::settle_record,
            commands::record::unsettle_record,
            commands::record::read_image_base64,
            commands::record::check_attachment_consistency,
            commands::record::cleanup_orphan_images,
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
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    show_main_window(app_handle);
                }
            }
        });
}
