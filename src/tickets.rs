use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, query};
use std::sync::Arc;

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Ticket {
    id: i64,
    name: String,
    description: String,
    //comments: Vec<Comment>,
}

#[derive(Deserialize, Debug)]
pub struct TicketCreate {
    name: String,
    description: String,
}

pub async fn get_tickets(State(pool): State<Arc<SqlitePool>>) -> Json<Vec<Ticket>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, description FROM tickets ORDER BY id DESC
        "#,
    )
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let tickets = rows
        .into_iter()
        .map(|t| Ticket {
            id: t.get("id"),
            name: t.get("name"),
            description: t.get("description"),
        })
        .collect();

    Json(tickets)
}
pub async fn create_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<TicketCreate>,
) -> Json<Ticket> {
    let new_data = format!(
        "INSERT INTO tickets (name, description)
        VALUES ('{:?}', '{:?}')
        RETURNING id;",
        payload.name, payload.description,
    );
    let result = query(new_data.as_str()).fetch_one(&*pool).await.unwrap();
    Json(Ticket {
        id: result.get("id"),
        name: payload.name,
        description: payload.description,
    })
}
