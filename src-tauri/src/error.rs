use serde::Serialize;
use std::fmt;

/// Application-level error type with structured categories.
/// Serializes as a plain string for frontend compatibility.
#[derive(Debug)]
pub enum AppError {
    /// Database query/connection error
    Database(String),
    /// Requested resource not found
    NotFound(String),
    /// Input validation failed
    Validation(String),
    /// Authentication/authorization error
    Auth(String),
    /// Filesystem or I/O error
    Io(String),
    /// Internal or unexpected error
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Database(msg)
            | AppError::NotFound(msg)
            | AppError::Validation(msg)
            | AppError::Auth(msg)
            | AppError::Io(msg)
            | AppError::Internal(msg) => write!(f, "{}", msg),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl std::error::Error for AppError {}

// ===== From implementations for automatic conversion with ? =====

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        // Map UNIQUE constraint violations to a more specific variant
        let msg = e.to_string();
        if msg.contains("UNIQUE") {
            AppError::Validation(msg)
        } else {
            AppError::Database(msg)
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Internal(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Internal(s.to_string())
    }
}

/// Convenience constructors
impl AppError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        AppError::Validation(msg.into())
    }

    pub fn auth(msg: impl Into<String>) -> Self {
        AppError::Auth(msg.into())
    }

    pub fn io(msg: impl Into<String>) -> Self {
        AppError::Io(msg.into())
    }
}
