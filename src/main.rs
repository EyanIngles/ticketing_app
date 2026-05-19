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
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Serialize, Deserialize)]
struct Ticket {
    id: u32,
    name: String,
    description: String,
    comments: Vec<Comment>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Comment {
    id: u32,
    text: String,
}

#[derive(Deserialize)]
struct TicketCreate {
    name: String,
    description: String,
}

#[derive(Deserialize)]
struct CommentCreate {
    text: String,
}

type AppState = Arc<Mutex<Vec<Ticket>>>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let state: AppState = Arc::new(Mutex::new(vec![]));

    let app = Router::new()
        .route("/tickets", get(get_tickets))
        .route("/tickets", post(create_ticket))
        .route("/tickets/:ticket_id", put(edit_ticket))
        .route("/tickets/:ticket_id", delete(delete_ticket))
        .route("/tickets/:id/comments", post(add_comment))
        .with_state(state)
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
async fn get_tickets(State(state): State<AppState>) -> Json<Vec<Ticket>> {
    let tickets = state.lock().await;
    Json(tickets.clone())
}

async fn create_ticket(
    State(state): State<AppState>,
    Json(payload): Json<TicketCreate>,
) -> Json<Ticket> {
    let mut tickets = state.lock().await;
    let new_ticket = Ticket {
        id: (tickets.len() as u32) + 1,
        name: payload.name,
        description: payload.description,
        comments: vec![],
    };
    tickets.push(new_ticket.clone());
    Json(new_ticket)
}

async fn edit_ticket(State(state): State<AppState>, Json(payload): Json<TicketCreate>)
// -> Json<Ticket> {
{
    println!("editing ticket name or description");
}

async fn delete_ticket(State(state): State<AppState>, Json(payload): Json<TicketCreate>)
// -> Json<Ticket> {
{
    println!("deleting ticket name or description");
}

async fn add_comment(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(payload): Json<CommentCreate>,
) -> Result<Json<Comment>, StatusCode> {
    let mut tickets = state.lock().await;
    if let Some(ticket) = tickets.iter_mut().find(|t| t.id == id) {
        let new_comment = Comment {
            id: (ticket.comments.len() as u32) + 1,
            text: payload.text,
        };
        ticket.comments.push(new_comment.clone());
        Ok(Json(new_comment))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
