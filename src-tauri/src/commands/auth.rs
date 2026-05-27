use crate::db::DbState;
use crate::models::{LoginResult, User};
use chrono::{Duration, Utc};
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn get_app_data_path(db: State<'_, DbState>) -> Result<String, String> {
    Ok(db.app_data_dir.to_string_lossy().to_string())
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
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("用户名和密码不能为空".into());
    }

    let needs_init = db.needs_init().await.map_err(|e| e.to_string())?;
    if !needs_init {
        return Err("系统已初始化，无法重复创建管理员".into());
    }

    let hash = bcrypt::hash(password.as_bytes(), 12).map_err(|e| e.to_string())?;

    sqlx::query("INSERT INTO users (username, password_hash) VALUES (?1, ?2)")
        .bind(username.trim())
        .bind(&hash)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn login(
    db: State<'_, DbState>,
    login_attempts: State<'_, crate::LoginAttempts>,
    username: String,
    password: String,
    remember: bool,
) -> Result<LoginResult, String> {
    // Check lockout
    {
        let attempts = login_attempts.0.lock().unwrap();
        if let Some((count, until)) = attempts.get(&username) {
            if *count >= 5 && Utc::now() < *until {
                let remaining = (*until - Utc::now()).num_minutes();
                return Err(format!(
                    "账户已被锁定，请 {} 分钟后再试",
                    remaining.max(1)
                ));
            }
        }
    }

    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE username = ?1")
            .bind(&username)
            .fetch_optional(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    let (user_id, password_hash) = match row {
        Some(r) => r,
        None => {
            record_failed_attempt(&login_attempts, &username);
            return Err("用户名或密码错误".into());
        }
    };

    let valid = bcrypt::verify(password.as_bytes(), &password_hash).unwrap_or(false);
    if !valid {
        record_failed_attempt(&login_attempts, &username);
        return Err("用户名或密码错误".into());
    }

    // Clear failed attempts on success
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
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    let user = User {
        id: user_id,
        username,
        created_at: String::new(),
    };

    Ok(LoginResult { user, token })
}

fn record_failed_attempt(attempts: &crate::LoginAttempts, username: &str) {
    let mut map = attempts.0.lock().unwrap();
    let entry = map
        .entry(username.to_string())
        .or_insert((0, Utc::now()));
    entry.0 += 1;
    if entry.0 >= 5 {
        entry.1 = Utc::now() + Duration::minutes(15);
    }
}

#[tauri::command]
pub async fn logout(db: State<'_, DbState>, token: String) -> Result<(), String> {
    sqlx::query("DELETE FROM sessions WHERE token = ?1")
        .bind(&token)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn validate_session(db: State<'_, DbState>, token: String) -> Result<User, String> {
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT u.id, u.username, s.expires_at FROM users u
         INNER JOIN sessions s ON s.user_id = u.id
         WHERE s.token = ?1",
    )
    .bind(&token)
    .fetch_optional(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    let (id, username, expires_at) = row.ok_or("会话无效")?;

    let expires = chrono::NaiveDateTime::parse_from_str(&expires_at, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "日期解析错误".to_string())?;
    let now = Utc::now().naive_utc();

    if now > expires {
        sqlx::query("DELETE FROM sessions WHERE token = ?1")
            .bind(&token)
            .execute(&db.pool)
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
pub async fn list_users(db: State<'_, DbState>) -> Result<Vec<User>, String> {
    let users: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, username, created_at FROM users ORDER BY id")
            .fetch_all(&db.pool)
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

#[tauri::command]
pub async fn create_user(
    db: State<'_, DbState>,
    username: String,
    password: String,
) -> Result<(), String> {
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("用户名和密码不能为空".into());
    }

    let hash = bcrypt::hash(password.as_bytes(), 12).map_err(|e| e.to_string())?;

    sqlx::query("INSERT INTO users (username, password_hash) VALUES (?1, ?2)")
        .bind(username.trim())
        .bind(&hash)
        .execute(&db.pool)
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

#[tauri::command]
pub async fn delete_user(
    db: State<'_, DbState>,
    user_id: i64,
    current_user_id: i64,
) -> Result<(), String> {
    if user_id == current_user_id {
        return Err("不能删除当前登录的用户".into());
    }

    let result = sqlx::query("DELETE FROM users WHERE id = ?1")
        .bind(user_id)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("用户不存在".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn change_password(
    db: State<'_, DbState>,
    user_id: i64,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    if new_password.trim().is_empty() {
        return Err("新密码不能为空".into());
    }

    let row: Option<(String,)> =
        sqlx::query_as("SELECT password_hash FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(&db.pool)
            .await
            .map_err(|e| e.to_string())?;

    let (hash,) = row.ok_or("用户不存在")?;

    let valid = bcrypt::verify(old_password.as_bytes(), &hash).unwrap_or(false);
    if !valid {
        return Err("旧密码错误".into());
    }

    let new_hash = bcrypt::hash(new_password.as_bytes(), 12).map_err(|e| e.to_string())?;

    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
