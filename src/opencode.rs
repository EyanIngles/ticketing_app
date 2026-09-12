use crate::constants::OPENCODE_DISPATCH_PROMPT;
use serde::Deserialize;
use serde_json::json;
use sqlx::{Row, SqlitePool};

pub async fn dispatch_new_ticket(pool: &SqlitePool, ticket_id: i64) {
    if let Err(err) = dispatch_inner(pool, ticket_id).await {
        eprintln!("opencode dispatch ticket {ticket_id}: {err}");
        let _ = mark_ticket_failed(pool, ticket_id, &err).await;
    }
}

async fn dispatch_inner(pool: &SqlitePool, ticket_id: i64) -> Result<(), String> {
    let Some(opc_config) = OpenCodeConfig::from_env() else {
        println!("opencode: skipped (OPENCODE_BASE_URL not set)");
        return Ok(());
    };

    let ticket_context = load_ticket_context(pool, ticket_id)
        .await
        .ok_or_else(|| "ticket not found".to_string())?;

    set_status(pool, ticket_id, "running").await?;

    let session_id = create_session(&opc_config, &ticket_context.ticket_name).await?;
    save_session(pool, ticket_id, &session_id, &opc_config.model).await?;

    let prompt = build_prompt_with_id(&ticket_context, &opc_config, ticket_id);
    prompt_async(&opc_config, &session_id, &prompt).await?;
    Ok(())
}

struct OpenCodeConfig {
    base_url: String,
    username: String,
    password: String,
    model: String,
    agent_name: String,
    agent_role: String,
}

impl OpenCodeConfig {
    fn from_env() -> Option<Self> {
        let base_url = env_nonempty("OPENCODE_BASE_URL")?;
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            username: env_nonempty("OPENCODE_SERVER_USERNAME").unwrap_or_else(|| "opencode".into()),
            password: env_nonempty("OPENCODE_SERVER_PASSWORD").unwrap_or_default(),
            model: env_nonempty("LYRA_AGENT_DEFAULT_MODEL").unwrap_or_else(|| "Grok4.6".into()),
            agent_name: env_nonempty("LYRA_AGENT_NAME").unwrap_or_else(|| "Mark".into()),
            agent_role: env_nonempty("LYRA_AGENT_ROLE").unwrap_or_else(|| "Engineer".into()),
        })
    }
}

struct TicketContext {
    ticket_name: String,
    ticket_description: String,
    project_name: String,
    project_description: String,
}

fn env_nonempty(key: &str) -> Option<String> {
    dotenv::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn build_prompt_with_id(
    ticket_context: &TicketContext,
    opc_config: &OpenCodeConfig,
    ticket_id: i64,
) -> String {
    OPENCODE_DISPATCH_PROMPT
        .replace("{agent_name}", &opc_config.agent_name)
        .replace("{agent_role}", &opc_config.agent_role)
        .replace("{project_name}", &ticket_context.project_name)
        .replace("{project_description}", &ticket_context.project_description)
        .replace("{ticket_id}", &ticket_id.to_string())
        .replace("{ticket_name}", &ticket_context.ticket_name)
        .replace("{ticket_description}", &ticket_context.ticket_description)
}

async fn load_ticket_context(pool: &SqlitePool, ticket_id: i64) -> Option<TicketContext> {
    let row = sqlx::query(
        r#"SELECT t."name" AS ticket_name, t."description" AS ticket_description,
                  COALESCE(p."name", '') AS project_name,
                  COALESCE(p."description", '') AS project_description
           FROM tickets t
           LEFT JOIN projects p ON p."id" = t."project_id"
           WHERE t."id" = $1"#,
    )
    .bind(ticket_id)
    .fetch_optional(pool)
    .await
    .ok()??;

    Some(TicketContext {
        ticket_name: row.get("ticket_name"),
        ticket_description: row.get("ticket_description"),
        project_name: row.get("project_name"),
        project_description: row.get("project_description"),
    })
}

async fn set_status(pool: &SqlitePool, ticket_id: i64, status: &str) -> Result<(), String> {
    sqlx::query(
        r#"UPDATE tickets SET "status" = $1, "updated_at" = datetime('now') WHERE "id" = $2"#,
    )
    .bind(status)
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn save_session(
    pool: &SqlitePool,
    ticket_id: i64,
    session_id: &str,
    model: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"UPDATE tickets SET "opencode_session_id" = $1, "last_model" = $2, "updated_at" = datetime('now')
           WHERE "id" = $3"#,
    )
    .bind(session_id)
    .bind(model)
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_ticket_failed(pool: &SqlitePool, ticket_id: i64, err: &str) -> Result<(), String> {
    set_status(pool, ticket_id, "failed").await?;
    sqlx::query(
        r#"INSERT INTO comments ("ticket_id", "text", "author_name", "author_type", "format")
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(ticket_id)
    .bind(format!("OpenCode dispatch failed: {err}"))
    .bind("system")
    .bind("system")
    .bind("markdown")
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Deserialize)]
struct SessionResponse {
    id: String,
}

async fn create_session(opc_config: &OpenCodeConfig, title: &str) -> Result<String, String> {
    let http = reqwest::Client::new();
    let url = format!("{}/session", opc_config.base_url);
    let mut req = http.post(&url).json(&json!({ "title": title }));
    if !opc_config.password.is_empty() {
        req = req.basic_auth(&opc_config.username, Some(&opc_config.password));
    }
    let response = req.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("create session HTTP {}", response.status()));
    }
    let session: SessionResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(session.id)
}

async fn prompt_async(
    opc_config: &OpenCodeConfig,
    session_id: &str,
    text: &str,
) -> Result<(), String> {
    let http = reqwest::Client::new();
    let url = format!("{}/session/{session_id}/prompt_async", opc_config.base_url);
    let mut req = http.post(&url).json(&json!({
        "parts": [{ "type": "text", "text": text }]
    }));
    if !opc_config.password.is_empty() {
        req = req.basic_auth(&opc_config.username, Some(&opc_config.password));
    }
    let response = req.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() && response.status().as_u16() != 204 {
        return Err(format!("prompt_async HTTP {}", response.status()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_project_and_task() {
        let ticket_context = TicketContext {
            ticket_name: "Add dark mode".into(),
            ticket_description: "Toggle on settings".into(),
            project_name: "Lyra".into(),
            project_description: "Ticketing".into(),
        };
        let opc_config = OpenCodeConfig {
            base_url: "http://127.0.0.1:4096".into(),
            username: "opencode".into(),
            password: String::new(),
            model: "Grok4.6".into(),
            agent_name: "Mark".into(),
            agent_role: "Engineer".into(),
        };
        let prompt = build_prompt_with_id(&ticket_context, &opc_config, 12);
        assert!(prompt.contains("Project: Lyra"));
        assert!(prompt.contains("Project description: Ticketing"));
        assert!(prompt.contains("Ticket #12: Add dark mode"));
        assert!(prompt.contains("Toggle on settings"));
        assert!(prompt.contains("You are Mark, role Engineer"));
        assert!(prompt.contains("awaiting_you"));
    }
}
