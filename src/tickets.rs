use axum::extract::Path;
use axum::{extract::State, response::Json};
use axum_server::bind;
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
#[derive(Deserialize, Serialize, Debug)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password: String,
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
                    id: c.get("id"),
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

pub async fn get_user_details(
    State(pool): State<Arc<SqlitePool>>,
    Path(email): Path<String>,
) -> User {
    let user = sqlx::query("SELECT * FROM users WHERE email = ($1)")
        .bind(email.clone())
        .fetch_one(&*pool)
        .await
        .unwrap();

    User {
        id: user.get("id"),
        email: user.get("email"),
        password: user.get("password"),
    }
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

async fn migrate_users_into_tickets(pool: SqlitePool) -> bool {
    match sqlx::query(
        "
        SELECT name FROM sqlite_master WHERE type='table' AND name='users';",
    )
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(_)) => {
            println!("Users table exists -> returning false to now allow a migration to happen.");
            false
        }
        Ok(None) => {
            println!("Users table does not exist -> returning true to run migration");
            true
        }
        Err(e) => {
            println!(
                "Error when checking table exists: {:?}, running migration to ensure table is created",
                e
            );
            true
        }
    }
}

async fn migration_down(pool: SqlitePool) {
    sqlx::query("DROP TABLE ($1);")
        .bind("users")
        .execute(&pool)
        .await
        .unwrap();
}

pub async fn migrate_db_2(pool: &SqlitePool) {
    if migrate_users_into_tickets(pool.clone()).await {
        //this here was giving us the correct value.
        //this should have been true if the DB has
        //been already migrated.
        let users_migration = sqlx::query(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL, password TEXT NOT NULL);
            ",
        )
        .execute(&pool.clone())
        .await;
        match users_migration {
            Ok(_) => println!("successfully migrated db to add users."),
            Err(_) => {
                migration_down(pool.clone()).await; // this code was casuing a crash.
            }
        }
    } else {
        println!("Migration already completed.");
    }
}
