use axum::extract::Path;
use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, query};
use std::sync::Arc;

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Ticket {
    id: i64,
    name: String,
    description: String,
    project_id: i64,
    comments: Vec<Comment>,
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

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Comment {
    pub id: i64,
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
        SELECT id, name, description, project_id FROM tickets ORDER BY id DESC 
        "#,
    )
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let all_comments = sqlx::query("SELECT id, ticket_id, text FROM comments")
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
                .map(|c| Comment {
                    id: c.get("id"),
                    text: c.get("text"),
                })
                .collect();

            Ticket {
                id: t.get("id"),
                name: t.get("name"),
                description: t.get("description"),
                project_id: t.get("project_id"),
                comments: ticket_comments,
            }
        })
        .collect();
    tickets
}
pub async fn create_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<TicketCreate>,
) -> Ticket {
    let result = query(
        "INSERT INTO tickets (name, description, project_id)
        VALUES ($1, $2, $3)
        RETURNING id;",
    )
    .bind(payload.name.clone())
    .bind(payload.description.clone())
    .bind(payload.project_id.clone())
    .fetch_one(&*pool)
    .await
    .unwrap();
    Ticket {
        id: result.get("id"),
        name: payload.name,
        description: payload.description,
        project_id: payload.project_id,
        comments: vec![],
    }
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
