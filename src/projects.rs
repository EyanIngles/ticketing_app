use crate::tickets::Ticket;
use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Project {
    id: i32,
    name: String,
    description: String,
    tickets: Vec<Ticket>,
}
#[derive(serde::Deserialize)]
pub struct CreateProject {
    name: String,
    description: String,
}

pub async fn fetch_projects(State(pool): State<Arc<SqlitePool>>) -> Vec<Project> {
    let rows = sqlx::query("SELECT id, name, description FROM projects ORDER BY id DESC")
        .fetch_all(&*pool)
        .await
        .unwrap_or_default();

    let projects: Vec<Project> = rows
        .into_iter()
        .map(|p| Project {
            id: p.get("id"),
            name: p.get("name"),
            description: p.get("description"),
            tickets: vec![], // ← Important fix
        })
        .collect();

    projects
}

pub async fn create_project(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<CreateProject>,
) -> StatusCode {
    let result_query = sqlx::query("INSERT INTO projects (name, description) VALUES($1, $2);")
        .bind(payload.name)
        .bind(payload.description)
        .execute(&*pool)
        .await;

    match result_query {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            println!("Err: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
