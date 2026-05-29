use crate::db::DbState;
use crate::models::{LoginResult, User};
use chrono::{Duration, Utc};
use tauri::State;
use tracing::{info, warn};
use uuid::Uuid;

// ===== Internal helpers (take &DbState, testable without Tauri) =====

pub async fn do_init_admin(db: &DbState, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("用户名和密码不能为空".into());
    }
    let needs = db.needs_init().await.map_err(|e| e.to_string())?;
    if !needs {
        return Err("系统已初始化，无法重复创建管理员".into());
    }
    let hash = bcrypt::hash(password.as_bytes(), 12).map_err(|e| e.to_string())?;
    let pool = db.get_pool().await?;
    sqlx::query("INSERT INTO users (username, password_hash) VALUES (?1, ?2)")
        .bind(username.trim())
        .bind(&hash)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    info!("管理员账号已创建: {}", username.trim());
    Ok(())
}

pub async fn do_list_users(db: &DbState) -> Result<Vec<User>, String> {
    let pool = db.get_pool().await?;
    let users: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, username, created_at FROM users ORDER BY id")
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(users
        .into_iter()
        .map(|(id, username, created_at)| User {
            id,
            username,
            created_at,
        })
        .collect())
}

pub async fn do_create_user(db: &DbState, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("用户名和密码不能为空".into());
    }
    let hash = bcrypt::hash(password.as_bytes(), 12).map_err(|e| e.to_string())?;
    let pool = db.get_pool().await?;
    sqlx::query("INSERT INTO users (username, password_hash) VALUES (?1, ?2)")
        .bind(username.trim())
        .bind(&hash)
        .execute(&pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "用户名已存在".into()
            } else {
                e.to_string()
            }
        })?;
    Ok(())
}

pub async fn do_delete_user(
    db: &DbState,
    user_id: i64,
    current_user_id: i64,
) -> Result<(), String> {
    if user_id == current_user_id {
        return Err("不能删除当前登录的用户".into());
    }
    let pool = db.get_pool().await?;
    let result = sqlx::query("DELETE FROM users WHERE id = ?1")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("用户不存在".into());
    }
    Ok(())
}

pub async fn do_change_password(
    db: &DbState,
    user_id: i64,
    old_password: &str,
    new_password: &str,
) -> Result<(), String> {
    if new_password.trim().is_empty() {
        return Err("新密码不能为空".into());
    }
    let pool = db.get_pool().await?;
    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = ?1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let (hash,) = row.ok_or("用户不存在")?;
    if !bcrypt::verify(old_password.as_bytes(), &hash).unwrap_or(false) {
        return Err("旧密码错误".into());
    }
    let new_hash = bcrypt::hash(new_password.as_bytes(), 12).map_err(|e| e.to_string())?;
    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ===== Tauri command wrappers =====

#[tauri::command]
pub async fn get_app_data_path(db: State<'_, DbState>, token: String) -> Result<String, String> {
    db.validate_token(&token).await?;
    Ok(db.app_data_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn check_db_integrity(db: State<'_, DbState>) -> Result<Option<String>, String> {
    Ok(db.get_integrity_error().await)
}

#[tauri::command]
pub async fn needs_init(db: State<'_, DbState>) -> Result<bool, String> {
    db.needs_init().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn init_admin(
    db: State<'_, DbState>,
    username: String,
    password: String,
) -> Result<(), String> {
    do_init_admin(&db, &username, &password).await
}

#[tauri::command]
pub async fn login(
    db: State<'_, DbState>,
    login_attempts: State<'_, crate::LoginAttempts>,
    username: String,
    password: String,
    remember: bool,
) -> Result<LoginResult, String> {
    {
        let attempts = login_attempts.0.lock().unwrap();
        if let Some((count, until)) = attempts.get(&username) {
            if *count >= 5 && Utc::now() < *until {
                let remaining = (*until - Utc::now()).num_minutes();
                return Err(format!("账户已被锁定，请 {} 分钟后再试", remaining.max(1)));
            }
        }
    }

    let pool = db.get_pool().await?;
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE username = ?1")
            .bind(&username)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (user_id, password_hash) = match row {
        Some(r) => r,
        None => {
            warn!("登录失败: 用户 {} 不存在", username);
            record_failed_attempt(&login_attempts, &username);
            return Err("用户名或密码错误".into());
        }
    };

    if !bcrypt::verify(password.as_bytes(), &password_hash).unwrap_or(false) {
        warn!("登录失败: 用户 {} 密码错误", username);
        record_failed_attempt(&login_attempts, &username);
        return Err("用户名或密码错误".into());
    }

    {
        login_attempts.0.lock().unwrap().remove(&username);
    }

    let token = Uuid::new_v4().to_string();
    let expires_at = if remember {
        Utc::now() + Duration::days(7)
    } else {
        Utc::now() + Duration::hours(24)
    };

    sqlx::query("INSERT INTO sessions (user_id, token, expires_at) VALUES (?1, ?2, ?3)")
        .bind(user_id)
        .bind(&token)
        .bind(expires_at.format("%Y-%m-%d %H:%M:%S").to_string())
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    info!("用户 {} 登录成功 (remember={})", username, remember);

    Ok(LoginResult {
        user: User {
            id: user_id,
            username,
            created_at: String::new(),
        },
        token,
    })
}

fn record_failed_attempt(attempts: &crate::LoginAttempts, username: &str) {
    let mut map = attempts.0.lock().unwrap();
    let entry = map.entry(username.to_string()).or_insert((0, Utc::now()));
    entry.0 += 1;
    if entry.0 >= 5 {
        entry.1 = Utc::now() + Duration::minutes(15);
        warn!("用户 {} 登录尝试失败 {} 次，已锁定 15 分钟", username, entry.0);
    }
}

#[tauri::command]
pub async fn logout(db: State<'_, DbState>, token: String) -> Result<(), String> {
    let user_id = db.validate_token(&token).await?;
    let pool = db.get_pool().await?;
    sqlx::query("DELETE FROM sessions WHERE token = ?1")
        .bind(&token)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    info!("用户 {} 已登出", user_id);
    Ok(())
}

#[tauri::command]
pub async fn validate_session(db: State<'_, DbState>, token: String) -> Result<User, String> {
    let pool = db.get_pool().await?;
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT u.id, u.username, s.expires_at FROM users u
         INNER JOIN sessions s ON s.user_id = u.id
         WHERE s.token = ?1",
    )
    .bind(&token)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (id, username, expires_at) = row.ok_or("会话无效")?;
    let expires = chrono::NaiveDateTime::parse_from_str(&expires_at, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "日期解析错误".to_string())?;

    if Utc::now().naive_utc() > expires {
        sqlx::query("DELETE FROM sessions WHERE token = ?1")
            .bind(&token)
            .execute(&pool)
            .await
            .ok();
        return Err("会话已过期".into());
    }

    Ok(User {
        id,
        username,
        created_at: String::new(),
    })
}

#[tauri::command]
pub async fn list_users(db: State<'_, DbState>, token: String) -> Result<Vec<User>, String> {
    db.validate_token(&token).await?;
    do_list_users(&db).await
}

#[tauri::command]
pub async fn create_user(
    db: State<'_, DbState>,
    token: String,
    username: String,
    password: String,
) -> Result<(), String> {
    db.validate_token(&token).await?;
    do_create_user(&db, &username, &password).await
}

#[tauri::command]
pub async fn delete_user(
    db: State<'_, DbState>,
    token: String,
    user_id: i64,
) -> Result<(), String> {
    let current_user_id = db.validate_token(&token).await?;
    do_delete_user(&db, user_id, current_user_id).await
}

#[tauri::command]
pub async fn change_password(
    db: State<'_, DbState>,
    token: String,
    user_id: i64,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let current_user_id = db.validate_token(&token).await?;
    if user_id != current_user_id {
        return Err("只能修改自己的密码".into());
    }
    do_change_password(&db, user_id, &old_password, &new_password).await
}

#[tauri::command]
pub async fn admin_reset_password(
    db: State<'_, DbState>,
    token: String,
    target_user_id: i64,
    new_password: String,
) -> Result<(), String> {
    let current_user_id = db.validate_token(&token).await?;
    // Only the initial admin (user id=1) may reset other users' passwords.
    if current_user_id != 1 {
        return Err("仅管理员可重置他人密码".into());
    }
    if new_password.trim().is_empty() {
        return Err("新密码不能为空".into());
    }
    let new_hash = bcrypt::hash(new_password.as_bytes(), 12).map_err(|e| e.to_string())?;
    let pool = db.get_pool().await?;
    let result = sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
        .bind(&new_hash)
        .bind(target_user_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("用户不存在".into());
    }
    Ok(())
}
