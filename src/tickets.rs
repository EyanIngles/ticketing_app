use axum::extract::Path;
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

pub async fn get_tickets(State(pool): State<Arc<SqlitePool>>) -> Vec<Ticket> {
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
    tickets
}
pub async fn create_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<TicketCreate>,
) -> Ticket {
    let result = query(
        "INSERT INTO tickets (name, description)
        VALUES ($1, $2)
        RETURNING id;",
    )
    .bind(payload.name.clone())
    .bind(payload.description.clone())
    .fetch_one(&*pool)
    .await
    .unwrap();
    Ticket {
        id: result.get("id"),
        name: payload.name,
        description: payload.description,
    }
}

pub async fn delete_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i32>,
) -> bool {
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
