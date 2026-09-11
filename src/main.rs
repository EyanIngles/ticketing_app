mod auth;
mod db;
mod jwt;
mod migrate;
mod opencode;
mod projects;
mod request_log;
mod seed;
mod tickets;
use projects::{CreateProject, Project};

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::Json,
    routing::{delete, get, post, put},
};
use axum_macros::{debug_handler, debug_middleware};
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use sqlx::SqlitePool;
use std::sync::Arc;
use tickets::{LoginRequest, Ticket, TicketCreate};
use tower_http::services::ServeFile;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let pool = match db::init_db().await {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("Database init or migration failed: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = seed::seed(&pool).await {
        eprintln!("seed failed: {err}");
        std::process::exit(1);
    }

    let pool = Arc::new(pool);

    let app = Router::new()
        .route("/projects", get(fetch_projects))
        .route("/projects", post(create_project))
        .route("/tickets", get(get_all_tickets))
        .route("/tickets", post(create_ticket))
        .route(
            "/tickets/:ticket_id",
            get(tickets::get_ticket).delete(delete_ticket),
        )
        .route("/tickets/:ticket_id/comments", post(tickets::add_comment))
        .route(
            "/tickets/:ticket_id/actions/request_pr",
            post(tickets::request_pr),
        )
        .route(
            "/tickets/:ticket_id/actions/close",
            post(tickets::close_ticket),
        )
        .route("/login", post(user_login))
        .route("/oauth/authorize", post(auth::oauth_authorize))
        .route("/oauth/token", post(auth::oauth_token))
        .route("/current_user", get(auth::current_user))
        .route(
            "/tickets/:ticket_id/comments/:comment_id",
            delete(delete_comment),
        )
        .fallback_service(ServeFile::new(
            "../lyra-frontend/target/dx/lyra-frontend/release/web/public/index.html",
        ))
        .with_state(pool.clone())
        .layer(middleware::from_fn_with_state(
            pool.clone(),
            request_log::log_request,
        ))
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
