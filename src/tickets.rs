use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Ticket {
    id: i64,
    name: String,
    description: String,
    //comments: Vec<Comment>,
}

pub async fn get_tickets(State(pool): State<Arc<SqlitePool>>) -> Json<Vec<Ticket>> {
    let tickets = sqlx::query_as!(
        Ticket,
        r#"
        SELECT id, name, description FROM tickets ORDER BY id DESC
        "#,
    )
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    Json(tickets)
}
