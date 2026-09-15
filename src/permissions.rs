use crate::auth::{AuthError, require_human};
use crate::constants::{PERMISSION_APPROVED, PERMISSION_DENIED, PERMISSION_PENDING};
use crate::opencode;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct PermissionQuery {
    pub status: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PermissionRequest {
    pub id: i64,
    pub ticket_id: i64,
    pub author_name: String,
    pub author_type: String,
    pub author_role: String,
    pub model: String,
    pub display: String,
    pub tool: String,
    pub payload: String,
    pub status: String,
    pub opencode_session_id: String,
    pub permission_id: String,
    pub is_used: bool,
}

fn opt_text(row: &SqliteRow, column: &str) -> String {
    row.try_get::<Option<String>, _>(column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn from_row(row: &SqliteRow) -> PermissionRequest {
    let author_name = opt_text(row, "author_name");
    let author_type = opt_text(row, "author_type");
    let model = opt_text(row, "model");
    let display = if author_type.eq_ignore_ascii_case("Agent") && !model.is_empty() {
        format!("{author_name}:{model}")
    } else {
        author_name.clone()
    };
    PermissionRequest {
        id: row.get("id"),
        ticket_id: row.try_get("ticket_id").unwrap_or(0),
        author_name,
        author_type,
        author_role: opt_text(row, "author_role"),
        model,
        display,
        tool: opt_text(row, "tool"),
        payload: opt_text(row, "payload"),
        status: opt_text(row, "status"),
        opencode_session_id: opt_text(row, "opencode_session_id"),
        permission_id: opt_text(row, "permission_id"),
        is_used: row.try_get::<i64, _>("is_used").unwrap_or(0) != 0,
    }
}

pub async fn list_permissions(
    State(pool): State<Arc<SqlitePool>>,
    headers: HeaderMap,
    Query(query): Query<PermissionQuery>,
) -> Result<Json<Vec<PermissionRequest>>, (StatusCode, Json<AuthError>)> {
    require_human(&headers)?;
    let status = query
        .status
        .unwrap_or_else(|| PERMISSION_PENDING.to_string());
    let rows = sqlx::query(
        r#"SELECT "id", "ticket_id", "author_name", "author_type", "author_role", "model",
                  "tool", "payload", "status", "opencode_session_id", "permission_id", "is_used"
           FROM permission_requests WHERE "status" = $1 ORDER BY "id" DESC"#,
    )
    .bind(&status)
    .fetch_all(&*pool)
    .await
    .map_err(|_| server_error())?;
    Ok(Json(rows.iter().map(from_row).collect()))
}

pub async fn approve_permission(
    State(pool): State<Arc<SqlitePool>>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<PermissionRequest>, (StatusCode, Json<AuthError>)> {
    require_human(&headers)?;
    decide(&pool, id, true).await
}

pub async fn deny_permission(
    State(pool): State<Arc<SqlitePool>>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<PermissionRequest>, (StatusCode, Json<AuthError>)> {
    require_human(&headers)?;
    decide(&pool, id, false).await
}

fn server_error() -> (StatusCode, Json<AuthError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AuthError {
            error: "server_error".into(),
        }),
    )
}

async fn decide(
    pool: &SqlitePool,
    id: i64,
    allow: bool,
) -> Result<Json<PermissionRequest>, (StatusCode, Json<AuthError>)> {
    let new_status = if allow {
        PERMISSION_APPROVED
    } else {
        PERMISSION_DENIED
    };
    let result = sqlx::query(
        r#"UPDATE permission_requests
           SET "status" = $1, "is_used" = 1
           WHERE "id" = $2 AND "status" = $3 AND "is_used" = 0"#,
    )
    .bind(new_status)
    .bind(id)
    .bind(PERMISSION_PENDING)
    .execute(pool)
    .await
    .map_err(|_| server_error())?;

    if result.rows_affected() == 0 {
        let exists = sqlx::query_scalar::<_, i64>(
            r#"SELECT 1 FROM permission_requests WHERE "id" = $1 LIMIT 1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| server_error())?;
        return Err(if exists.is_none() {
            (
                StatusCode::NOT_FOUND,
                Json(AuthError {
                    error: "not_found".into(),
                }),
            )
        } else {
            (
                StatusCode::CONFLICT,
                Json(AuthError {
                    error: "already_decided".into(),
                }),
            )
        });
    }

    let updated = sqlx::query(
        r#"SELECT "id", "ticket_id", "author_name", "author_type", "author_role", "model",
                  "tool", "payload", "status", "opencode_session_id", "permission_id", "is_used"
           FROM permission_requests WHERE "id" = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|_| server_error())?;
    let current = from_row(&updated);

    let skip_opencode = if current.permission_id.is_empty() {
        false
    } else {
        let other_used = sqlx::query_scalar::<_, i64>(
            r#"SELECT 1 FROM permission_requests
               WHERE "permission_id" = $1 AND "is_used" = 1 AND "id" != $2
               LIMIT 1"#,
        )
        .bind(&current.permission_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| server_error())?;
        other_used.is_some()
    };

    if !skip_opencode
        && let Err(err) = opencode::respond_to_permission(
            &current.opencode_session_id,
            &current.permission_id,
            allow,
        )
        .await
    {
        eprintln!("opencode permission reply: {err}");
    }

    Ok(Json(current))
}

pub async fn insert_request(
    pool: &SqlitePool,
    ticket_id: i64,
    agent_id: i64,
    author_name: &str,
    author_role: &str,
    model: &str,
    tool: &str,
    payload: &str,
    permission_id: &str,
) -> Result<i64, sqlx::Error> {
    let session: String = sqlx::query_scalar(
        r#"SELECT COALESCE("opencode_session_id", '') FROM tickets WHERE "id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();

    let row = sqlx::query(
        r#"INSERT INTO permission_requests
           ("ticket_id", "author_id", "author_name", "author_type", "author_role", "model",
            "opencode_session_id", "permission_id", "tool", "payload", "status")
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           RETURNING "id""#,
    )
    .bind(ticket_id)
    .bind(agent_id)
    .bind(author_name)
    .bind("Agent")
    .bind(author_role)
    .bind(model)
    .bind(&session)
    .bind(permission_id)
    .bind(tool)
    .bind(payload)
    .bind(PERMISSION_PENDING)
    .fetch_one(pool)
    .await?;
    Ok(row.get("id"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jwt::encode_access_token;
    use crate::migrate::run_migrations;
    use axum::http::HeaderValue;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> SqlitePool {
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
        sqlx::query(r#"INSERT INTO projects ("name", "description") VALUES ('Lyra', 'App')"#)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            r#"INSERT INTO tickets ("name", "description", "project_id", "status")
               VALUES ('Task', 'Do it', 1, 'running')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn jwt_headers(user_type: &str, role: &str) -> HeaderMap {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
        }
        let token =
            encode_access_token("test-jwt-secret", "eyan", user_type, role, "Eyan").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers
    }

    fn human_headers() -> HeaderMap {
        jwt_headers("Human", "Owner")
    }

    fn agent_headers() -> HeaderMap {
        jwt_headers("Agent", "Engineer")
    }

    async fn insert_pending(pool: &SqlitePool) -> i64 {
        insert_request(
            pool,
            1,
            2,
            "Mark",
            "Engineer",
            "Grok4.6",
            "bash",
            "rm -rf /tmp/x",
            "perm-1",
        )
        .await
        .unwrap()
    }

    async fn row_is_used(pool: &SqlitePool, id: i64) -> (i64, String, i64) {
        let row = sqlx::query(
            r#"SELECT COUNT(*) AS n, "status", "is_used" FROM permission_requests WHERE "id" = $1"#,
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
        (row.get("n"), row.get("status"), row.get("is_used"))
    }

    #[tokio::test]
    async fn list_and_deny_pending_permission() {
        let pool = setup().await;
        let id = insert_pending(&pool).await;

        let list = list_permissions(
            State(Arc::new(pool.clone())),
            human_headers(),
            Query(PermissionQuery {
                status: Some("pending".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].display, "Mark:Grok4.6");
        assert_eq!(list[0].tool, "bash");
        assert!(!list[0].is_used);

        let denied = deny_permission(State(Arc::new(pool.clone())), Path(id), human_headers())
            .await
            .unwrap();
        assert_eq!(denied.status, PERMISSION_DENIED);
        assert!(denied.is_used);
        let (n, status, is_used) = row_is_used(&pool, id).await;
        assert_eq!(n, 1);
        assert_eq!(status, PERMISSION_DENIED);
        assert_eq!(is_used, 1);

        let pending = list_permissions(
            State(Arc::new(pool)),
            human_headers(),
            Query(PermissionQuery {
                status: Some("pending".into()),
            }),
        )
        .await
        .unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn list_without_jwt_is_unauthorized() {
        let pool = setup().await;
        let err = list_permissions(
            State(Arc::new(pool)),
            HeaderMap::new(),
            Query(PermissionQuery { status: None }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn human_with_empty_role_can_list() {
        let pool = setup().await;
        insert_pending(&pool).await;
        let list = list_permissions(
            State(Arc::new(pool)),
            jwt_headers("Human", ""),
            Query(PermissionQuery { status: None }),
        )
        .await
        .unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn agent_jwt_is_forbidden_on_permission_routes() {
        let pool = setup().await;
        let id = insert_pending(&pool).await;
        let headers = agent_headers();

        let list_err = list_permissions(
            State(Arc::new(pool.clone())),
            headers.clone(),
            Query(PermissionQuery { status: None }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(list_err.0, StatusCode::FORBIDDEN);
        assert_eq!(list_err.1.0.error, "not_human");

        let approve_err =
            approve_permission(State(Arc::new(pool.clone())), Path(id), headers.clone())
                .await
                .err()
                .unwrap();
        assert_eq!(approve_err.0, StatusCode::FORBIDDEN);
        assert_eq!(approve_err.1.0.error, "not_human");

        let deny_err = deny_permission(State(Arc::new(pool.clone())), Path(id), headers)
            .await
            .err()
            .unwrap();
        assert_eq!(deny_err.0, StatusCode::FORBIDDEN);
        assert_eq!(deny_err.1.0.error, "not_human");

        let (n, status, is_used) = row_is_used(&pool, id).await;
        assert_eq!(n, 1);
        assert_eq!(status, PERMISSION_PENDING);
        assert_eq!(is_used, 0);
    }

    #[tokio::test]
    async fn approve_twice_is_conflict_and_row_remains() {
        let pool = setup().await;
        let id = insert_pending(&pool).await;

        let approved = approve_permission(State(Arc::new(pool.clone())), Path(id), human_headers())
            .await
            .unwrap();
        assert_eq!(approved.status, PERMISSION_APPROVED);
        assert!(approved.is_used);

        let second = approve_permission(State(Arc::new(pool.clone())), Path(id), human_headers())
            .await
            .err()
            .unwrap();
        assert_eq!(second.0, StatusCode::CONFLICT);
        assert_eq!(second.1.0.error, "already_decided");

        let (n, status, is_used) = row_is_used(&pool, id).await;
        assert_eq!(n, 1);
        assert_eq!(status, PERMISSION_APPROVED);
        assert_eq!(is_used, 1);
    }

    #[tokio::test]
    async fn deny_then_approve_is_conflict_and_row_remains() {
        let pool = setup().await;
        let id = insert_pending(&pool).await;

        let denied = deny_permission(State(Arc::new(pool.clone())), Path(id), human_headers())
            .await
            .unwrap();
        assert_eq!(denied.status, PERMISSION_DENIED);
        assert!(denied.is_used);

        let second = approve_permission(State(Arc::new(pool.clone())), Path(id), human_headers())
            .await
            .err()
            .unwrap();
        assert_eq!(second.0, StatusCode::CONFLICT);
        assert_eq!(second.1.0.error, "already_decided");

        let (n, status, is_used) = row_is_used(&pool, id).await;
        assert_eq!(n, 1);
        assert_eq!(status, PERMISSION_DENIED);
        assert_eq!(is_used, 1);
    }
}
