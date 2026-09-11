use crate::jwt::decode_access_token;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use rand::RngExt;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Instant;

pub async fn log_request(
    State(pool): State<Arc<SqlitePool>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = path_only(req.uri().path(), req.uri().query());
    let user_name = optional_user_name(req.headers());
    let request_id = request_id();

    let started = Instant::now();
    let response = next.run(req).await;
    let status = i64::from(response.status().as_u16());
    let duration_ms = started.elapsed().as_millis() as i64;

    let pool = pool.clone();
    tokio::spawn(async move {
        let _ = insert_log(
            &pool,
            &request_id,
            &method,
            &path,
            status,
            duration_ms,
            user_name.as_deref(),
        )
        .await;
    });

    response
}

fn path_only(path: &str, query: Option<&str>) -> String {
    let _ = query;
    path.to_string()
}

fn request_id() -> String {
    let bytes: [u8; 16] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn optional_user_name(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?;
    let secret = dotenv::var("JWT_SECRET").ok()?;
    decode_access_token(&secret, token).ok().map(|c| c.username)
}

pub async fn insert_log(
    pool: &SqlitePool,
    request_id: &str,
    method: &str,
    path: &str,
    status: i64,
    duration_ms: i64,
    user_name: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO request_logs ("request_id", "method", "path", "status", "duration_ms", "user_name")
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(request_id)
    .bind(method)
    .bind(path)
    .bind(status)
    .bind(duration_ms)
    .bind(user_name)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_id INTEGER NOT NULL,
                text TEXT NOT NULL
            );
            CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT
            );
            CREATE TABLE system (version TEXT NOT NULL, date TEXT NOT NULL);
            CREATE TABLE tickets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                project_id INTEGER REFERENCES projects(id)
            );
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL,
                password TEXT NOT NULL
            );
            INSERT INTO system (version, date) VALUES ('0.1.0', '12 July 2026');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    #[test]
    fn path_drops_query_string() {
        assert_eq!(
            path_only("/oauth/token", Some("code=secret")),
            "/oauth/token"
        );
        assert_eq!(path_only("/tickets/1", None), "/tickets/1");
    }

    #[tokio::test]
    async fn insert_log_stores_status_and_path_not_query() {
        let pool = setup_pool().await;
        insert_log(&pool, "abc", "GET", "/tickets/1", 200, 12, Some("eyan"))
            .await
            .unwrap();
        let row = sqlx::query(
            r#"SELECT "path", "status", "user_name" FROM request_logs WHERE "request_id" = 'abc'"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let path: String = sqlx::Row::get(&row, "path");
        let status: i64 = sqlx::Row::get(&row, "status");
        let name: String = sqlx::Row::get(&row, "user_name");
        assert_eq!(path, "/tickets/1");
        assert_eq!(status, 200);
        assert_eq!(name, "eyan");
    }
}
