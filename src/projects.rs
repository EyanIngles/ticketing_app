use crate::{
    auth::{AuthError, require_human},
    tickets::Ticket,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
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
    headers: HeaderMap,
    Json(payload): Json<CreateProject>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    require_human(&headers)?;
    let result_query = sqlx::query("INSERT INTO projects (name, description) VALUES($1, $2);")
        .bind(payload.name)
        .bind(payload.description)
        .execute(&*pool)
        .await;

    match result_query {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(e) => {
            println!("Err: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    error: "server_error".into(),
                }),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jwt::encode_access_token;
    use axum::http::HeaderValue;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            r#"CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn jwt_headers(user_type: &str) -> HeaderMap {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
        }
        let token = encode_access_token(
            "test-jwt-secret",
            "test-user",
            user_type,
            "Owner",
            "Test User",
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers
    }

    fn payload() -> Json<CreateProject> {
        Json(CreateProject {
            name: "Lyra".into(),
            description: "Ticketing".into(),
        })
    }

    #[tokio::test]
    async fn create_project_without_jwt_is_unauthorized() {
        let pool = setup_pool().await;
        let err = create_project(State(Arc::new(pool)), HeaderMap::new(), payload())
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.error, "invalid_token");
    }

    #[tokio::test]
    async fn create_project_with_agent_jwt_is_forbidden() {
        let pool = setup_pool().await;
        let err = create_project(State(Arc::new(pool)), jwt_headers("Agent"), payload())
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert_eq!(err.1.error, "not_human");
    }

    #[tokio::test]
    async fn human_can_create_project() {
        let pool = setup_pool().await;
        let status = create_project(
            State(Arc::new(pool.clone())),
            jwt_headers("Human"),
            payload(),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::CREATED);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
