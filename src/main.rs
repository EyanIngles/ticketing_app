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
use sqlx::SqlitePool;
use std::sync::Arc;
use tickets::{Ticket, TicketCreate};
use tokio::sync::Mutex;

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize)]
struct Comment {
    id: i64,
    text: String,
}

#[derive(Deserialize)]
struct CommentCreate {
    text: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let pool = db::init_db()
        .await
        .expect("Err: Unable to initialise data base function.");

    let app = Router::new()
        .route("/tickets", get(get_all_tickets))
        .route("/tickets", post(create_ticket))
        //.route("/tickets/:ticket_id", put(edit_ticket))
        .route("/tickets/:ticket_id", delete(delete_ticket))
        //.route("/tickets/:id/comments", post(add_comment))
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
//}

//async fn delete_ticket(State(pool): State<Arc<SqlitePool>, Json(payload): Json<TicketCreate>)
// -> Json<Ticket> {
//{
//    println!("deleting ticket name or description");
//}

//async fn add_comment(
//    Path(id): Path<u32>,
//    State(pool): State<Arc<SqlitePool>,
//    Json(payload): Json<CommentCreate>,
//) -> Result<Json<Comment>, StatusCode> {
//    let mut tickets =
//    if let Some(ticket) = tickets.iter_mut().find(|t| t.id == id) {
//        let new_comment = Comment {
//            id: (ticket.comments.len() as u32) + 1,
//            text: payload.text,
//        };
//        ticket.comments.push(new_comment.clone());
//        Ok(Json(new_comment))
//    } else {
//        Err(StatusCode::NOT_FOUND)
//    }
//}
