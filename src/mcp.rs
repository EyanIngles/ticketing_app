use crate::auth::{AuthError, UserRow, require_agent};
use crate::tickets::{self, Ticket};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct JsonRpcRequest {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

pub async fn mcp_post(
    State(pool): State<Arc<SqlitePool>>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> Result<Json<Value>, (StatusCode, Json<AuthError>)> {
    let agent = require_agent(&pool, &headers).await?;
    let id = req.id.clone();
    let result = match req.method.as_str() {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "lyra", "version": "0.1.0" }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tool_defs() }),
        "tools/call" => call_tool(&pool, &agent, req.params).await?,
        "notifications/initialized" => {
            return Ok(Json(json!({
                "jsonrpc": req.jsonrpc.unwrap_or_else(|| "2.0".into()),
                "result": null
            })));
        }
        _ => {
            return Ok(Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not found" }
            })));
        }
    };
    Ok(Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })))
}

fn tool_defs() -> Value {
    json!([
        {
            "name": "lyra_get_ticket",
            "description": "Get a Lyra ticket and its comments",
            "inputSchema": {
                "type": "object",
                "properties": { "ticket_id": { "type": "integer" } },
                "required": ["ticket_id"]
            }
        },
        {
            "name": "lyra_add_comment",
            "description": "Add a markdown comment on a ticket as this agent",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer" },
                    "text": { "type": "string" },
                    "model": { "type": "string" }
                },
                "required": ["ticket_id", "text"]
            }
        },
        {
            "name": "lyra_set_status",
            "description": "Set ticket status (e.g. awaiting_you, pending_review, failed)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer" },
                    "status": { "type": "string" }
                },
                "required": ["ticket_id", "status"]
            }
        },
        {
            "name": "lyra_set_pr_url",
            "description": "Store the GitHub PR URL and set status pending_review",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer" },
                    "url": { "type": "string" }
                },
                "required": ["ticket_id", "url"]
            }
        }
    ])
}

async fn call_tool(
    pool: &SqlitePool,
    agent: &UserRow,
    params: Option<Value>,
) -> Result<Value, (StatusCode, Json<AuthError>)> {
    let params = params.unwrap_or(json!({}));
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let text = match name {
        "lyra_get_ticket" => {
            let id = arg_i64(&args, "ticket_id")?;
            let ticket = load_ticket(pool, id).await?;
            serde_json::to_string_pretty(&ticket).unwrap_or_else(|_| "{}".into())
        }
        "lyra_add_comment" => {
            let id = arg_i64(&args, "ticket_id")?;
            let text = arg_str(&args, "text")?;
            let model = args
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| agent.default_model.clone());
            add_agent_comment(pool, agent, id, &text, &model).await?;
            format!("comment added on ticket {id}")
        }
        "lyra_set_status" => {
            let id = arg_i64(&args, "ticket_id")?;
            let status = arg_str(&args, "status")?;
            set_status(pool, id, &status).await?;
            format!("ticket {id} status {status}")
        }
        "lyra_set_pr_url" => {
            let id = arg_i64(&args, "ticket_id")?;
            let url = arg_str(&args, "url")?;
            set_pr_url(pool, id, &url).await?;
            format!("ticket {id} pr {url}")
        }
        _ => {
            return Ok(json!({
                "content": [{ "type": "text", "text": "unknown tool" }],
                "isError": true
            }));
        }
    };
    Ok(json!({
        "content": [{ "type": "text", "text": text }]
    }))
}

fn arg_i64(args: &Value, key: &str) -> Result<i64, (StatusCode, Json<AuthError>)> {
    args.get(key).and_then(|v| v.as_i64()).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: format!("missing {key}"),
            }),
        )
    })
}

fn arg_str(args: &Value, key: &str) -> Result<String, (StatusCode, Json<AuthError>)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: format!("missing {key}"),
                }),
            )
        })
}

async fn load_ticket(
    pool: &SqlitePool,
    ticket_id: i64,
) -> Result<Ticket, (StatusCode, Json<AuthError>)> {
    tickets::get_ticket(State(Arc::new(pool.clone())), Path(ticket_id))
        .await
        .map(|j| j.0)
        .map_err(|s| {
            (
                s,
                Json(AuthError {
                    error: "not_found".into(),
                }),
            )
        })
}

async fn add_agent_comment(
    pool: &SqlitePool,
    agent: &UserRow,
    ticket_id: i64,
    text: &str,
    model: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    sqlx::query(
        r#"INSERT INTO comments ("ticket_id", "text", "author_name", "author_type", "author_role", "model", "format")
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(ticket_id)
    .bind(text)
    .bind(&agent.name)
    .bind("Agent")
    .bind(&agent.role)
    .bind(model)
    .bind("markdown")
    .execute(pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;
    if !model.is_empty() {
        let _ = sqlx::query(
            r#"UPDATE tickets SET "last_model" = $1, "updated_at" = datetime('now') WHERE "id" = $2"#,
        )
        .bind(model)
        .bind(ticket_id)
        .execute(pool)
        .await;
    }
    Ok(())
}

async fn set_status(
    pool: &SqlitePool,
    ticket_id: i64,
    status: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    let result = sqlx::query(
        r#"UPDATE tickets SET "status" = $1, "updated_at" = datetime('now') WHERE "id" = $2"#,
    )
    .bind(status)
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;
    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "not_found".into(),
            }),
        ));
    }
    Ok(())
}

async fn set_pr_url(
    pool: &SqlitePool,
    ticket_id: i64,
    url: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    let result = sqlx::query(
        r#"UPDATE tickets SET "github_pr_url" = $1, "status" = $2, "updated_at" = datetime('now')
           WHERE "id" = $3"#,
    )
    .bind(url)
    .bind("pending_review")
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;
    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "not_found".into(),
            }),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::hash_password;
    use crate::migrate::run_migrations;
    use axum::http::HeaderValue;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> (SqlitePool, i64) {
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
        let hash = hash_password("agent-token").unwrap();
        sqlx::query(
            r#"INSERT INTO users ("email", "password", "username", "type", "role", "name", "default_model", "token_hash")
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind("mark@agent.lyra")
        .bind(&hash)
        .bind("mark-eng")
        .bind("Agent")
        .bind("Engineer")
        .bind("Mark")
        .bind("Grok4.6")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO tickets ("name", "description", "project_id", "status")
               VALUES ('Task', 'Do it', 1, 'queued')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let ticket_id: i64 = sqlx::query_scalar("SELECT id FROM tickets")
            .fetch_one(&pool)
            .await
            .unwrap();
        (pool, ticket_id)
    }

    fn agent_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-lyra-user", HeaderValue::from_static("mark-eng"));
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer agent-token"),
        );
        headers
    }

    #[tokio::test]
    async fn mcp_rejects_missing_auth() {
        let (pool, _) = setup().await;
        let err = mcp_post(
            State(Arc::new(pool)),
            HeaderMap::new(),
            Json(JsonRpcRequest {
                jsonrpc: Some("2.0".into()),
                id: Some(json!(1)),
                method: "tools/list".into(),
                params: None,
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mcp_agent_can_comment_and_set_status() {
        let (pool, ticket_id) = setup().await;
        let headers = agent_headers();
        let _ = mcp_post(
            State(Arc::new(pool.clone())),
            headers.clone(),
            Json(JsonRpcRequest {
                jsonrpc: Some("2.0".into()),
                id: Some(json!(1)),
                method: "tools/call".into(),
                params: Some(json!({
                    "name": "lyra_add_comment",
                    "arguments": { "ticket_id": ticket_id, "text": "## done" }
                })),
            }),
        )
        .await
        .unwrap();
        let _ = mcp_post(
            State(Arc::new(pool.clone())),
            headers,
            Json(JsonRpcRequest {
                jsonrpc: Some("2.0".into()),
                id: Some(json!(2)),
                method: "tools/call".into(),
                params: Some(json!({
                    "name": "lyra_set_status",
                    "arguments": { "ticket_id": ticket_id, "status": "awaiting_you" }
                })),
            }),
        )
        .await
        .unwrap();

        let ticket = tickets::get_ticket(State(Arc::new(pool)), Path(ticket_id))
            .await
            .unwrap();
        assert_eq!(ticket.status, "awaiting_you");
        assert_eq!(ticket.comments[0].display, "Mark:Grok4.6");
        assert_eq!(ticket.last_model, "Grok4.6");
    }
}
