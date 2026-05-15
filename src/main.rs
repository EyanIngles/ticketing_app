use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use axum_server::tls_rustls::RustlsConfig;
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
    let state: AppState = Arc::new(Mutex::new(vec![]));

    let app = Router::new()
        .route("/tickets", get(get_tickets))
        .route("/tickets", post(create_ticket))
        .route("/tickets/:id/comments", post(add_comment))
        .with_state(state)
        .layer(tower_http::cors::CorsLayer::permissive());

    // ================== HTTPS SETUP ==================
    // 1. Run this command first on your server machine:
    //    tailscale cert your-machine.tailnet.ts.net
    //
    // 2. Put the two generated files in the same folder as this binary

    let cert_path = "keys/eyans-macbook-pro.tailcb4684.ts.net.crt";
    let key_path = "keys/eyans-macbook-pro.tailcb4684.ts.net.key";

    println!("🔒 Loading Tailscale certificates...");
    let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .expect("Failed to load certificates. Make sure cert files exist!");

    let addr = "100.104.203.56:3000";
    println!("🚀 HTTPS Server running on https://{}", addr);
    println!("📱 Use in Swift: https://your-machine.tailnet.ts.net:3000");

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
