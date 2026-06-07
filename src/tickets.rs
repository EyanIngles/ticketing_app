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
    comments: Vec<Comment>,
}

#[derive(Deserialize, Debug)]
pub struct TicketCreate {
    name: String,
    description: String,
}

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Comment {
    pub id: i64,
    pub text: String,
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
                    id: ticket_id,
                    text: c.get("text"),
                })
                .collect();

            Ticket {
                id: t.get("id"),
                name: t.get("name"),
                description: t.get("description"),
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
        comments: vec![],
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

async fn migrate_comments_into_tickets(pool: SqlitePool) -> bool {
    let result = sqlx::query(
        "
        SELECT EXISTS (
            SELECT * FROM pragma_table_info($1)
            )",
    )
    .bind("comments")
    .fetch_all(&pool)
    .await;

    match result {
        Ok(_) => {
            println!("comment table call check passed");
            true
        }
        Err(_) => false,
    }
}

pub async fn migrate_db_1(pool: &SqlitePool) {
    if migrate_comments_into_tickets(pool.clone()).await {
        let comment_migration = sqlx::query(
            "CREATE TABLE comments(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_id INTEGER NOT NULL,
                text TEXT NOT NULL,

                FOREIGN KEY (ticket_id) REFERENCES tickets(id)
                )",
        )
        .execute(pool)
        .await;

        match comment_migration {
            Ok(result) => println!(
                "successfully migrated database with new comment table {:?}",
                result
            ),
            Err(e) => println!("Err: Unsuccessful migration attempt -> {:?}", e),
        }
    } else {
        println!("Migration already completed.");
    }
}
