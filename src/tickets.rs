use crate::db_down;
use argon2::Error;

use axum::extract::Path;
use axum::{extract::State, response::Json};
//use axum_server::bind;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteError, SqliteQueryResult};
use sqlx::{Row, SqlitePool, pool, query};
use std::ptr::null;
use std::sync::Arc;
use std::thread::current;

#[derive(Debug)]
struct System {
    version: String,
    date: String,
}

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

async fn is_version_up_to_date(pool: &SqlitePool) -> System {
    let current_version = sqlx::query("SELECT * FROM system;").fetch_one(*&pool).await;
    match current_version {
        Ok(Version) => {
            return System {
                version: Version.get("version"),
                date: Version.get("date"),
            };
        }
        Err(e) => {
            println!("Err: {:?}", e);
            return System {
                version: "0".to_string(),
                date: "".to_string(),
            };
        }
    }
}
async fn migration_down(pool: SqlitePool) {
    //db down.
}

async fn db_up(pool: SqlitePool) -> Result<SqliteQueryResult, sqlx::Error> {
    println!("db_up being activated... running query now.");
    let migration = sqlx::query(
        "CREATE TABLE IF NOT EXISTS system(version TEXT NOT NULL, date TEXT NOT NULL);
        CREATE IF NOT EXISTS TABLE projects(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        description TEXT); 
    ",
    )
    .execute(&pool)
    .await;

    let _ =
        sqlx::query("ALTER TABLE tickets ADD COLUMN project_id INTEGER REFERENCES projects(id);")
            .execute(&pool)
            .await?;
    let _ = sqlx::query("INSERT INTO system(version, date) VALUES($1, $2)")
        .bind("0.1.0") //2 digits each part of the version so
        //we have version 0.1.0 if we round
        //down but can go all the up to version
        //99.99.99: we have an additional 2 digits which gives us a value of 0 at the start.
        .bind("12 July 2026 - ~8:00PM")
        .execute(&pool)
        .await?;

    println!("Completed migration on DB.");

    Ok(migration?)
}

pub async fn migration_up(pool: &SqlitePool) {
    let current_system = is_version_up_to_date(&pool.clone()).await;
    let new_version = "0.1.0";
    println!("system return: {:?}", &current_system);
    if current_system.version != new_version || current_system.version == "0" {
        match db_up(pool.clone()).await {
            Ok(_) => println!("successfully migrated db to add users."),
            Err(_) => {
                migration_down(pool.clone()).await; // this code was casuing a crash.
            }
        }
    } else if current_system.version == new_version {
        println!("System version up to date.");
    } else {
        println!("potential error: no version matching, starting db roll back with db_down...");
        migration_down(pool.clone()).await;
        println!("Database reverted successfully.");
        //db_down activate
    }
}
