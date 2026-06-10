mod db;
mod tickets;

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::SqlitePool;
use std::sync::Arc;
use tickets::{Comment, Ticket, TicketCreate};
use tokio::sync::Mutex;

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

    tickets::migrate_db_2(&pool).await;

    let app = Router::new()
        .route("/tickets", get(get_all_tickets))
        .route("/tickets", post(create_ticket))
        //.route("/tickets/:ticket_id", put(edit_ticket))
        .route("/tickets/:ticket_id", delete(delete_ticket))
        .route("/tickets/:ticket_id/comments", post(add_comment))
        .route(
            "/tickets/:ticket_id/comments/:comment_id",
            delete(delete_comment),
        )
        .with_state(Arc::new(pool))
        .layer(tower_http::cors::CorsLayer::permissive());

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
