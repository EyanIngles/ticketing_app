mod db;
mod db_down;
mod projects;
mod tickets;
use projects::{CreateProject, Project};

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use axum_macros::{debug_handler, debug_middleware};
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use serde::Deserialize;
use sqlx::Row;
use sqlx::SqlitePool;
use std::sync::Arc;
use tickets::{Comment, LoginRequest, Ticket, TicketCreate, User};
use tower_http::services::ServeFile;
//use tokio::sync::Mutex;

#[derive(Deserialize, Debug)]
struct CommentCreate {
    text: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let pool = db::init_db()
        .await
        .expect("Err: Unable to initialise data base function.");

    println!("🔧 Running migration...");

    tickets::migration_up(&pool).await;

    let app = Router::new()
        .route("/projects", get(fetch_projects))
        .route("/projects", post(create_project))
        .route("/tickets", get(get_all_tickets))
        .route("/tickets", post(create_ticket))
        //.route("/tickets/:ticket_id", put(edit_ticket))
        .route("/tickets/:ticket_id", delete(delete_ticket))
        .route("/tickets/:ticket_id/comments", post(add_comment))
        .route("/login", post(user_login))
        .route(
            "/tickets/:ticket_id/comments/:comment_id",
            delete(delete_comment),
        )
        .fallback_service(ServeFile::new(
            "../lyra-frontend/target/dx/lyra-frontend/release/web/public/index.html",
        ))
        .with_state(Arc::new(pool))
        .layer(tower_http::cors::CorsLayer::permissive());
    //.layer(GovernorLayer::new(*governor_config))

    // ================== HTTPS SETUP ==================
    // 1. Run this command first on your server machine:
    //    tailscale cert your-machine.tailnet.ts.net
    //
    // 2. Put the two generated files in the same folder as this binary

    let cert_path = dotenv::var("TAILSCALE_CERT_PATH").unwrap();
    let key_path = dotenv::var("TAILSCALE_KEY_PATH").unwrap();

    println!("🔒 Loading Tailscale certificates...");
    let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .expect("Failed to load certificates. Make sure cert files exist!");

    let addr = dotenv::var("SERVER_IP").unwrap();
    println!("🚀 HTTPS Server running on https://{}", addr);

    axum_server::bind_rustls(addr.parse().unwrap(), tls_config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// ================== Handlers (same as before) ==================
async fn get_all_tickets(State(pool): State<Arc<SqlitePool>>) -> Json<Vec<Ticket>> {
    let tickets = tickets::get_tickets(State(pool)).await;
    Json(tickets)
}

async fn fetch_projects(State(pool): State<Arc<SqlitePool>>) -> Json<Vec<Project>> {
    let projects = projects::fetch_projects(State(pool)).await;
    Json(projects)
}
#[debug_handler]
async fn create_project(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<CreateProject>,
) -> StatusCode {
    let project = projects::create_project(State(pool), Json(payload)).await;
    project
}

async fn _encrypt_password_for_storage(_password: String) -> String {
    "hi".to_string()
}

async fn encrypt_password_and_verify(_password: String) -> bool {
    true
}

async fn user_login(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<LoginRequest>,
) -> StatusCode {
    //if true, return the user, if or user doesnt exist, return false boolean.
    // call function to exists
    let user = tickets::get_user_details(State(pool), Json(payload)).await;
    if user {
        return StatusCode::FOUND;
    } else {
        return StatusCode::NO_CONTENT;
    }
}

async fn create_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<TicketCreate>,
) -> Json<Ticket> {
    //println!("{:?}", Json(payload));
    let ticket = tickets::create_ticket(State(pool), Json(payload)).await;
    Json(ticket)
}

async fn delete_ticket(
    State(pool): State<Arc<SqlitePool>>,
    Path(ticket_id): Path<i32>,
) -> StatusCode {
    let ticket = tickets::delete_ticket(State(pool), Path(ticket_id)).await;
    match ticket {
        true => StatusCode::NO_CONTENT,
        false => StatusCode::NOT_FOUND,
    }
}

//async fn edit_ticket(State(pool): State<Arc<SqlitePool>>, Json(payload): Json<TicketCreate>)
// -> Json<Ticket> {
//{
//    println!("editing ticket name or description");
//}}

async fn add_comment(
    Path(ticket_id): Path<u32>,
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<CommentCreate>,
) -> Result<Json<Comment>, StatusCode> {
    let record = sqlx::query("INSERT INTO comments (ticket_id, text) VALUES ($1, $2) RETURNING id")
        .bind(ticket_id)
        .bind(&payload.text)
        .fetch_one(&*pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let new_comment = Comment {
        id: record.get("id"), // ← This is the fix
        text: payload.text,
    };

    Ok(Json(new_comment))
}

async fn delete_comment(
    Path((_tickket_id, comment_id)): Path<(i32, i32)>,
    State(pool): State<Arc<SqlitePool>>,
) -> StatusCode {
    println!(
        "Delete comment function has beenr recieved.. waiting on completion. ..please wait..."
    );
    let result = sqlx::query("DELETE FROM comments WHERE id = ($1)")
        .bind(comment_id)
        .execute(&*pool)
        .await;

    match result {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::NOT_FOUND,
    }
}
