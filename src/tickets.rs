use crate::auth::{AuthError, require_claim, require_human};
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, query, sqlite::SqliteRow};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Ticket {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub project_id: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub github_pr_url: String,
    #[serde(default)]
    pub last_model: String,
    pub comments: Vec<Comment>,
}

#[derive(Deserialize, Debug)]
pub struct TicketCreate {
    name: String,
    description: String,
    project_id: i64,
}

#[derive(Deserialize, Debug)]
pub struct LoginRequest {
    pub email: String,
    password: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Comment {
    pub id: i64,
    pub text: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_type: String,
    #[serde(default)]
    pub author_role: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub display: String,
}

#[derive(Deserialize, Debug)]
pub struct CommentCreate {
    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct SetStatusRequest {
    pub status: String,
}

const HUMAN_TICKET_STATUSES: &[&str] = &[
    "open",
    "awaiting_you",
    "pending_review",
    "closed",
    "failed",
    "cancelled",
];
#[derive(Deserialize, Serialize, Debug)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password: String,
}

pub async fn get_tickets(State(pool): State<Arc<SqlitePool>>) -> Vec<Ticket> {
    let rows = sqlx::query(
        r#"
        SELECT "id", "name", "description", "project_id", "status", "github_pr_url", "last_model"
        FROM tickets ORDER BY id DESC 
        "#,
    )
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let all_comments = sqlx::query(
        r#"SELECT "id", "ticket_id", "text", "author_name", "author_type", "author_role", "model", "format" FROM comments"#,
    )
        .fetch_all(&*pool)
        .await
        .unwrap_or_default();

    let tickets = rows
        .into_iter()
        .map(|t| {
            let ticket_id: i64 = t.get("id");

            let ticket_comments = all_comments
                .iter()
                .filter(|c| c.get::<i64, _>("ticket_id") == ticket_id)
                .map(comment_from_row)
                .collect();

            ticket_from_row(&t, ticket_comments)
        })
        .collect();
    tickets
}
fn server_error() -> (StatusCode, Json<AuthError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AuthError {
            error: "server_error".into(),
        }),
    )
}

pub async fn create_ticket(
    State(pool): State<Arc<SqlitePool>>,
    headers: HeaderMap,
    Json(payload): Json<TicketCreate>,
) -> Result<Ticket, (StatusCode, Json<AuthError>)> {
    require_claim(&headers)?;
    let result = query(
        r#"INSERT INTO tickets ("name", "description", "project_id", "status")
        VALUES ($1, $2, $3, $4)
        RETURNING "id""#,
    )
    .bind(payload.name.clone())
    .bind(payload.description.clone())
    .bind(payload.project_id)
    .bind("queued")
    .fetch_one(&*pool)
    .await
    .unwrap();
    let ticket = Ticket {
        id: result.get("id"),
        name: payload.name,
        description: payload.description,
        project_id: payload.project_id,
        status: "queued".into(),
        github_pr_url: String::new(),
        last_model: String::new(),
        comments: vec![],
    };
    let pool = pool.clone();
    let ticket_id = ticket.id;
    tokio::spawn(async move {
        crate::opencode::dispatch_new_ticket(&pool, ticket_id).await;
    });
    Ok(ticket)
}

fn opt_text(row: &SqliteRow, column: &str) -> String {
    row.try_get::<Option<String>, _>(column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn display_for(name: &str, user_type: &str, model: &str) -> String {
    if user_type.eq_ignore_ascii_case("Agent") && !model.is_empty() {
        format!("{name}:{model}")
    } else {
        name.to_string()
    }
}

fn comment_from_row(c: &SqliteRow) -> Comment {
    let author_name = opt_text(c, "author_name");
    let author_type = opt_text(c, "author_type");
    let author_role = opt_text(c, "author_role");
    let model = opt_text(c, "model");
    let format = {
        let f = opt_text(c, "format");
        if f.is_empty() { "markdown".into() } else { f }
    };
    let display = display_for(&author_name, &author_type, &model);
    Comment {
        id: c.get("id"),
        text: c.get("text"),
        author_name,
        author_type,
        author_role,
        model,
        format,
        display,
    }
}

fn ticket_from_row(t: &SqliteRow, comments: Vec<Comment>) -> Ticket {
    Ticket {
        id: t.get("id"),
        name: t.get("name"),
        description: t.get("description"),
        project_id: t.try_get("project_id").unwrap_or(0),
        status: {
            let s = opt_text(t, "status");
            if s.is_empty() { "queued".into() } else { s }
        },
        github_pr_url: opt_text(t, "github_pr_url"),
        last_model: opt_text(t, "last_model"),
        comments,
    }
}

async fn comments_for_ticket(pool: &SqlitePool, ticket_id: i64) -> Vec<Comment> {
    sqlx::query(
        r#"SELECT "id", "ticket_id", "text", "author_name", "author_type", "author_role", "model", "format"
           FROM comments WHERE "ticket_id" = $1 ORDER BY "id""#,
    )
    .bind(ticket_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .iter()
    .map(comment_from_row)
    .collect()
}

pub async fn get_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
    let row = sqlx::query(
        r#"SELECT "id", "name", "description", "project_id", "status", "github_pr_url", "last_model"
           FROM tickets WHERE "id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_optional(&*pool)
    .await
    .map_err(|_| server_error())?;
    let Some(row) = row else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "not_found".into(),
            }),
        ));
    };
    let comments = comments_for_ticket(&pool, ticket_id).await;
    Ok(Json(ticket_from_row(&row, comments)))
}

pub async fn add_comment(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<CommentCreate>,
) -> Result<Json<Comment>, (StatusCode, Json<AuthError>)> {
    let claim = require_claim(&headers)?;
    if payload.text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: "empty_text".into(),
            }),
        ));
    }
    let record = sqlx::query(
        r#"INSERT INTO comments ("ticket_id", "text", "author_name", "author_type", "author_role", "format")
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING "id", "text", "author_name", "author_type", "author_role", "model", "format""#,
    )
    .bind(ticket_id)
    .bind(&payload.text)
    .bind(&claim.name)
    .bind(&claim.user_type)
    .bind(&claim.role)
    .bind("markdown")
    .fetch_one(&*pool)
    .await
    .map_err(|_| server_error())?;
    Ok(Json(comment_from_row(&record)))
}

pub async fn deploy_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
    require_human(&headers)?;
    let row = sqlx::query(
        r#"SELECT "id", "name", "description", "project_id", "status", "github_pr_url", "last_model"
           FROM tickets WHERE "id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_optional(&*pool)
    .await
    .map_err(|_| server_error())?;
    let Some(row) = row else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "not_found".into(),
            }),
        ));
    };
    let comments = comments_for_ticket(&pool, ticket_id).await;
    let ticket = ticket_from_row(&row, comments);
    if !ticket.status.eq_ignore_ascii_case("pending_review") {
        return Err((
            StatusCode::CONFLICT,
            Json(AuthError {
                error: "not_ready".into(),
            }),
        ));
    }
    let pool_spawn = pool.clone();
    tokio::spawn(async move {
        crate::deploy::run_deploy(&pool_spawn, ticket_id).await;
    });
    Ok(Json(ticket))
}

pub async fn request_pr(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
    require_claim(&headers)?;
    set_ticket_status(&pool, ticket_id, "pr_opening").await
}

pub async fn close_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
    require_claim(&headers)?;
    set_ticket_status(&pool, ticket_id, "closed").await
}

pub async fn set_status(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<SetStatusRequest>,
) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
    require_human(&headers)?;
    let status = HUMAN_TICKET_STATUSES
        .iter()
        .copied()
        .find(|allowed| payload.status.eq_ignore_ascii_case(allowed))
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: "invalid_status".into(),
                }),
            )
        })?;
    set_ticket_status(&pool, ticket_id, status).await
}

async fn set_ticket_status(
    pool: &SqlitePool,
    ticket_id: i64,
    status: &str,
) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
    let result = sqlx::query(
        r#"UPDATE tickets SET "status" = $1, "updated_at" = datetime('now') WHERE "id" = $2"#,
    )
    .bind(status)
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|_| server_error())?;
    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "not_found".into(),
            }),
        ));
    }
    let row = sqlx::query(
        r#"SELECT "id", "name", "description", "project_id", "status", "github_pr_url", "last_model"
           FROM tickets WHERE "id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_one(pool)
    .await
    .map_err(|_| server_error())?;
    let comments = comments_for_ticket(pool, ticket_id).await;
    Ok(Json(ticket_from_row(&row, comments)))
}

pub async fn get_user_details(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<LoginRequest>,
) -> bool {
    let user = sqlx::query("SELECT * FROM users WHERE email = ($1)")
        .bind(payload.email.clone())
        .fetch_one(&*pool)
        .await;

    match user {
        Ok(_) => {
            println!("user exists");
            return true;
        }
        Err(e) => {
            println!("Err: {:?}", e);
            return false;
        }
    };
}

pub async fn delete_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i32>,
) -> bool {
    println!("Running comment checker here....");
    let exist = query(
        "SELECT EXISTS(
    SELECT 1 FROM comments WHERE ticket_id = ($1)
)",
    )
    .bind(ticket_id.clone())
    .fetch_one(&*pool)
    .await;

    let has_comments: bool = match exist {
        Ok(row) => row.get(0),
        Err(_) => false,
    };

    if has_comments {
        println!("Has comments attached to this ticket....");
    } else {
        println!("this ticket has no comments attached...");
    }
    let query_data = format!("DELETE FROM tickets WHERE id = {:?}", ticket_id.to_string());
    let result = sqlx::query(query_data.as_str()).execute(&*pool).await;
    match result {
        Ok(res) => {
            println!(
                "ticket successfully deleted - Ticket No: {:?} - response: {:?}",
                ticket_id, res
            );
            true
        }
        Err(e) => {
            println!(
                "Err: Unable to perform 'delete_ticket' function - Err: {:?}",
                e
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jwt::encode_access_token;
    use crate::migrate::run_migrations;
    use axum::http::HeaderValue;
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
        sqlx::query(r#"INSERT INTO projects ("name", "description") VALUES ('p', 'd')"#)
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

    #[tokio::test]
    async fn get_ticket_includes_markdown_comment_display() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        assert_eq!(ticket.status, "queued");

        sqlx::query(
            r#"INSERT INTO comments ("ticket_id", "text", "author_name", "author_type", "author_role", "model", "format")
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(ticket.id)
        .bind("## done")
        .bind("Mark")
        .bind("Agent")
        .bind("Engineer")
        .bind("Grok4.6")
        .bind("markdown")
        .execute(&pool)
        .await
        .unwrap();

        let got = get_ticket(State(Arc::new(pool)), Path(ticket.id))
            .await
            .unwrap();
        assert_eq!(got.comments.len(), 1);
        assert_eq!(got.comments[0].display, "Mark:Grok4.6");
        assert_eq!(got.comments[0].format, "markdown");
    }

    #[tokio::test]
    async fn request_pr_without_token_is_unauthorized() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        let err = request_pr(State(Arc::new(pool)), Path(ticket.id), HeaderMap::new())
            .await
            .err()
            .unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.0.error, "invalid_token");
    }

    #[tokio::test]
    async fn get_ticket_missing_is_not_found() {
        let pool = setup_pool().await;
        let err = get_ticket(State(Arc::new(pool)), Path(999))
            .await
            .err()
            .unwrap();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
        assert_eq!(err.1.0.error, "not_found");
    }

    #[tokio::test]
    async fn add_comment_empty_text_is_bad_request() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        let err = add_comment(
            State(Arc::new(pool)),
            Path(ticket.id),
            human_headers(),
            Json(CommentCreate {
                text: "   ".into(),
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert_eq!(err.1.0.error, "empty_text");
    }

    #[tokio::test]
    async fn add_comment_without_token_is_unauthorized() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        let err = add_comment(
            State(Arc::new(pool)),
            Path(ticket.id),
            HeaderMap::new(),
            Json(CommentCreate {
                text: "hello".into(),
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.0.error, "invalid_token");
    }

    #[tokio::test]
    async fn human_can_set_open_status() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        let got = set_status(
            State(Arc::new(pool)),
            Path(ticket.id),
            human_headers(),
            Json(SetStatusRequest {
                status: "open".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(got.status, "open");
    }

    #[tokio::test]
    async fn set_status_agent_jwt_is_forbidden() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        let err = set_status(
            State(Arc::new(pool)),
            Path(ticket.id),
            jwt_headers("Agent", "Engineer"),
            Json(SetStatusRequest {
                status: "open".into(),
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert_eq!(err.1.0.error, "not_human");
    }

    #[tokio::test]
    async fn set_status_queued_is_invalid() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            human_headers(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .unwrap();
        let err = set_status(
            State(Arc::new(pool)),
            Path(ticket.id),
            human_headers(),
            Json(SetStatusRequest {
                status: "queued".into(),
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert_eq!(err.1.0.error, "invalid_status");
    }

    #[tokio::test]
    async fn create_ticket_without_token_is_unauthorized() {
        let pool = setup_pool().await;
        let err = create_ticket(
            State(Arc::new(pool)),
            HeaderMap::new(),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.0.error, "invalid_token");
    }
}
