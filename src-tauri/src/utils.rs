/// Escape special characters for SQLite LIKE patterns (\, %, _).
/// Callers must use `ESCAPE '\\'` in their SQL queries.
pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}
