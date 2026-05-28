pub mod commands;
pub mod db;
pub mod models;

use db::{sqlite_options, DbState};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;

pub struct LoginAttempts(pub Mutex<HashMap<String, (u32, chrono::DateTime<chrono::Utc>)>>);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(LoginAttempts(Mutex::new(HashMap::new())))
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

            app.manage(db);
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
