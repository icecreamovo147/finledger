mod commands;
mod db;
mod models;

use db::DbState;
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
            let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

            let pool = tauri::async_runtime::block_on(async {
                SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect(&db_url)
                    .await
                    .expect("failed to connect to database")
            });

            let db = DbState::new(pool, app_dir);
            tauri::async_runtime::block_on(async {
                db.run_migrations().await.expect("failed to run migrations");
            });

            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::get_app_data_path,
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
            commands::record::list_records,
            commands::record::get_record,
            commands::record::update_record,
            commands::record::delete_record,
            commands::record::upload_image,
            commands::record::delete_image,
            commands::record::settle_record,
            commands::record::unsettle_record,
            commands::record::read_image_base64,
            commands::export::export_excel,
            commands::dashboard::get_dashboard_stats,
            commands::backup::backup_database,
            commands::backup::restore_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
