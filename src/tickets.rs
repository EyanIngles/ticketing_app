use crate::auth::require_claim;
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
pub async fn create_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<TicketCreate>,
) -> Ticket {
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
    Ticket {
        id: result.get("id"),
        name: payload.name,
        description: payload.description,
        project_id: payload.project_id,
        status: "queued".into(),
        github_pr_url: String::new(),
        last_model: String::new(),
        comments: vec![],
    }
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
) -> Result<Json<Ticket>, StatusCode> {
    let row = sqlx::query(
        r#"SELECT "id", "name", "description", "project_id", "status", "github_pr_url", "last_model"
           FROM tickets WHERE "id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_optional(&*pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(row) = row else {
        return Err(StatusCode::NOT_FOUND);
    };
    let comments = comments_for_ticket(&pool, ticket_id).await;
    Ok(Json(ticket_from_row(&row, comments)))
}

pub async fn add_comment(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<CommentCreate>,
) -> Result<Json<Comment>, StatusCode> {
    if payload.text.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let claim = require_claim(&headers).ok();
    let (author_name, author_type, author_role) = match &claim {
        Some(c) => (c.name.clone(), c.user_type.clone(), c.role.clone()),
        None => (String::new(), String::new(), String::new()),
    };
    let record = sqlx::query(
        r#"INSERT INTO comments ("ticket_id", "text", "author_name", "author_type", "author_role", "format")
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING "id", "text", "author_name", "author_type", "author_role", "model", "format""#,
    )
    .bind(ticket_id)
    .bind(&payload.text)
    .bind(&author_name)
    .bind(&author_type)
    .bind(&author_role)
    .bind("markdown")
    .fetch_one(&*pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(comment_from_row(&record)))
}

pub async fn request_pr(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Ticket>, (StatusCode, Json<crate::auth::AuthError>)> {
    require_claim(&headers)?;
    set_ticket_status(&pool, ticket_id, "pr_opening")
        .await
        .map_err(|s| {
            (
                s,
                Json(crate::auth::AuthError {
                    error: "not_found".into(),
                }),
            )
        })
}

pub async fn close_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Ticket>, (StatusCode, Json<crate::auth::AuthError>)> {
    require_claim(&headers)?;
    set_ticket_status(&pool, ticket_id, "closed")
        .await
        .map_err(|s| {
            (
                s,
                Json(crate::auth::AuthError {
                    error: "not_found".into(),
                }),
            )
        })
}

async fn set_ticket_status(
    pool: &SqlitePool,
    ticket_id: i64,
    status: &str,
) -> Result<Json<Ticket>, StatusCode> {
    let result = sqlx::query(
        r#"UPDATE tickets SET "status" = $1, "updated_at" = datetime('now') WHERE "id" = $2"#,
    )
    .bind(status)
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    let row = sqlx::query(
        r#"SELECT "id", "name", "description", "project_id", "status", "github_pr_url", "last_model"
           FROM tickets WHERE "id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_one(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
        sqlx::query(r#"INSERT INTO projects ("name", "description") VALUES ('p', 'd')"#)
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn get_ticket_includes_markdown_comment_display() {
        let pool = setup_pool().await;
        let ticket = create_ticket(
            State(Arc::new(pool.clone())),
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await;
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
            Json(TicketCreate {
                name: "Task".into(),
                description: "Do the thing".into(),
                project_id: 1,
            }),
        )
        .await;
        let err = request_pr(State(Arc::new(pool)), Path(ticket.id), HeaderMap::new())
            .await
            .err()
            .unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
    }
}
