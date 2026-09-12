use crate::jwt::{decode_access_token, encode_access_token, now_secs};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;

const CODE_TTL_SECS: u64 = 600;
const REFRESH_TTL_SECS: u64 = 60 * 60 * 24 * 30;

#[derive(Deserialize)]
pub struct AuthorizeRequest {
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

#[derive(Serialize)]
pub struct AuthorizeResponse {
    pub code: String,
}

#[derive(Deserialize, Clone)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: Option<String>,
    pub code: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct CurrentUserResponse {
    pub id: i64,
    pub username: String,
    #[serde(rename = "type")]
    pub user_type: String,
    pub role: String,
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Debug)]
pub struct AuthError {
    pub error: String,
}

pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password: String,
    pub user_type: String,
    pub role: String,
    pub name: String,
    pub email: String,
    pub token_hash: String,
    pub default_model: String,
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt_bytes: [u8; 16] = rand::rng().random();
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| e.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn random_hex() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn pkce_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn jwt_secret() -> Result<String, StatusCode> {
    dotenv::var("JWT_SECRET").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(|s| s.to_string())
}

pub fn require_claim(
    headers: &HeaderMap,
) -> Result<crate::jwt::Claim, (StatusCode, Json<AuthError>)> {
    let secret =
        jwt_secret().map_err(|_| auth_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?;
    let Some(token) = bearer_token(headers) else {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_token"));
    };
    decode_access_token(&secret, &token)
        .map_err(|_| auth_err(StatusCode::UNAUTHORIZED, "invalid_token"))
}

fn text_or_empty(row: &sqlx::sqlite::SqliteRow, column: &str) -> String {
    row.try_get::<Option<String>, _>(column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

async fn load_user_by_username(pool: &SqlitePool, username: &str) -> Option<UserRow> {
    let row = sqlx::query(
        r#"SELECT "id", "username", "password", "type", "role", "name", "email", "token_hash", "default_model"
           FROM users WHERE "username" = $1"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .ok()??;

    Some(UserRow {
        id: row.get("id"),
        username: text_or_empty(&row, "username"),
        password: row.get("password"),
        user_type: text_or_empty(&row, "type"),
        role: text_or_empty(&row, "role"),
        name: text_or_empty(&row, "name"),
        email: text_or_empty(&row, "email"),
        token_hash: text_or_empty(&row, "token_hash"),
        default_model: text_or_empty(&row, "default_model"),
    })
}

pub async fn require_agent(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> Result<UserRow, (StatusCode, Json<AuthError>)> {
    let username = headers
        .get("x-lyra-user")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| auth_err(StatusCode::UNAUTHORIZED, "invalid_token"))?;
    let token =
        bearer_token(headers).ok_or_else(|| auth_err(StatusCode::UNAUTHORIZED, "invalid_token"))?;
    let Some(user) = load_user_by_username(pool, &username).await else {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_token"));
    };
    if !user.user_type.eq_ignore_ascii_case("Agent") {
        return Err(auth_err(StatusCode::FORBIDDEN, "not_agent"));
    }
    let hash = if user.token_hash.is_empty() {
        &user.password
    } else {
        &user.token_hash
    };
    if !verify_password(&token, hash) {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_token"));
    }
    Ok(user)
}

fn auth_err(status: StatusCode, error: &str) -> (StatusCode, Json<AuthError>) {
    (
        status,
        Json(AuthError {
            error: error.to_string(),
        }),
    )
}

fn parse_expiry(raw: &str) -> Result<u64, (StatusCode, Json<AuthError>)> {
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| auth_err(StatusCode::UNAUTHORIZED, "invalid_grant"))?;
    if parsed == 0 {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_grant"));
    }
    Ok(parsed)
}

async fn client_exists(
    pool: &SqlitePool,
    client_id: &str,
) -> Result<bool, (StatusCode, Json<AuthError>)> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"SELECT 1 FROM oauth_clients WHERE "client_id" = $1 LIMIT 1"#,
    )
    .bind(client_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| auth_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?;
    Ok(row.is_some())
}

async fn delete_one(
    pool: &SqlitePool,
    sql: &str,
    bind: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    let result = sqlx::query(sql)
        .bind(bind)
        .execute(pool)
        .await
        .map_err(|_| auth_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?;
    if result.rows_affected() == 0 {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_grant"));
    }
    Ok(())
}

pub async fn oauth_authorize(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<AuthorizeRequest>,
) -> Result<Json<AuthorizeResponse>, (StatusCode, Json<AuthError>)> {
    if payload.code_challenge_method != "S256" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: "invalid_request".into(),
            }),
        ));
    }
    if !client_exists(&pool, &payload.client_id).await? {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_client"));
    }
    let Some(user) = load_user_by_username(&pool, &payload.username).await else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_grant".into(),
            }),
        ));
    };
    if !verify_password(&payload.password, &user.password) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_grant".into(),
            }),
        ));
    }

    let code = random_hex();
    let expires_at = (now_secs() + CODE_TTL_SECS).to_string();
    sqlx::query(
        r#"INSERT INTO oauth_codes ("client_id", "code", "code_challenge", "username", "expires_at")
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(&payload.client_id)
    .bind(&code)
    .bind(&payload.code_challenge)
    .bind(&payload.username)
    .bind(expires_at)
    .execute(&*pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;

    Ok(Json(AuthorizeResponse { code }))
}

pub async fn oauth_token(
    State(pool): State<Arc<SqlitePool>>,
    Json(payload): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, (StatusCode, Json<AuthError>)> {
    match payload.grant_type.as_str() {
        "authorization_code" => token_from_code(&pool, payload).await,
        "refresh_token" => token_from_refresh(&pool, payload).await,
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: "unsupported_grant_type".into(),
            }),
        )),
    }
}

async fn token_from_code(
    pool: &SqlitePool,
    payload: TokenRequest,
) -> Result<Json<TokenResponse>, (StatusCode, Json<AuthError>)> {
    let (Some(code), Some(verifier), Some(client_id)) =
        (payload.code, payload.code_verifier, payload.client_id)
    else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: "invalid_request".into(),
            }),
        ));
    };

    let row = sqlx::query(
        r#"SELECT "code_challenge", "username", "expires_at", "client_id"
           FROM oauth_codes WHERE "code" = $1"#,
    )
    .bind(&code)
    .fetch_optional(pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;

    let Some(row) = row else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_grant".into(),
            }),
        ));
    };

    let stored_client: String = row.get("client_id");
    if stored_client != client_id {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_client".into(),
            }),
        ));
    }

    let expires_at: String = row.get("expires_at");
    let expires_at = parse_expiry(&expires_at)?;
    if now_secs() > expires_at {
        match delete_one(pool, r#"DELETE FROM oauth_codes WHERE "code" = $1"#, &code).await {
            Err((StatusCode::INTERNAL_SERVER_ERROR, body)) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, body));
            }
            _ => return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_grant")),
        }
    }

    let challenge: String = row.get("code_challenge");
    if pkce_s256(&verifier) != challenge {
        return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_grant"));
    }

    let username: String = row.get("username");
    delete_one(pool, r#"DELETE FROM oauth_codes WHERE "code" = $1"#, &code).await?;

    issue_tokens(pool, &username).await
}

async fn token_from_refresh(
    pool: &SqlitePool,
    payload: TokenRequest,
) -> Result<Json<TokenResponse>, (StatusCode, Json<AuthError>)> {
    let Some(refresh) = payload.refresh_token else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: "invalid_request".into(),
            }),
        ));
    };
    let hash = sha256_hex(&refresh);
    let row = sqlx::query(
        r#"SELECT "user_id", "expires_at" FROM refresh_tokens WHERE "token_hash" = $1"#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;

    let Some(row) = row else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_grant".into(),
            }),
        ));
    };

    let expires_at: String = row.get("expires_at");
    let expires_at = parse_expiry(&expires_at)?;
    if now_secs() > expires_at {
        match delete_one(
            pool,
            r#"DELETE FROM refresh_tokens WHERE "token_hash" = $1"#,
            &hash,
        )
        .await
        {
            Err((StatusCode::INTERNAL_SERVER_ERROR, body)) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, body));
            }
            _ => return Err(auth_err(StatusCode::UNAUTHORIZED, "invalid_grant")),
        }
    }

    let user_id: i64 = row.get("user_id");
    let username =
        sqlx::query_scalar::<_, Option<String>>(r#"SELECT "username" FROM users WHERE "id" = $1"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();

    let Some(username) = username else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_grant".into(),
            }),
        ));
    };

    delete_one(
        pool,
        r#"DELETE FROM refresh_tokens WHERE "token_hash" = $1"#,
        &hash,
    )
    .await?;

    issue_tokens(pool, &username).await
}

async fn issue_tokens(
    pool: &SqlitePool,
    username: &str,
) -> Result<Json<TokenResponse>, (StatusCode, Json<AuthError>)> {
    let secret = jwt_secret().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;
    let Some(user) = load_user_by_username(pool, username).await else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_grant".into(),
            }),
        ));
    };
    let access = encode_access_token(
        &secret,
        &user.username,
        &user.user_type,
        &user.role,
        &user.name,
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;
    let refresh = random_hex();
    let refresh_hash = sha256_hex(&refresh);
    let expires_at = (now_secs() + REFRESH_TTL_SECS).to_string();
    sqlx::query(
        r#"INSERT INTO refresh_tokens ("token_hash", "user_id", "expires_at") VALUES ($1, $2, $3)"#,
    )
    .bind(&refresh_hash)
    .bind(user.id)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;

    Ok(Json(TokenResponse {
        access_token: access,
        refresh_token: refresh,
        token_type: "Bearer".into(),
        expires_in: 3600,
    }))
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub async fn current_user(
    State(pool): State<Arc<SqlitePool>>,
    headers: HeaderMap,
) -> Result<Json<CurrentUserResponse>, (StatusCode, Json<AuthError>)> {
    let secret = jwt_secret().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".into(),
            }),
        )
    })?;
    let Some(token) = bearer_token(&headers) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_token".into(),
            }),
        ));
    };
    let claim = decode_access_token(&secret, &token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_token".into(),
            }),
        )
    })?;
    let Some(user) = load_user_by_username(&pool, &claim.username).await else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_token".into(),
            }),
        ));
    };
    Ok(Json(CurrentUserResponse {
        id: user.id,
        username: user.username,
        user_type: user.user_type,
        role: user.role,
        name: user.name,
        email: user.email,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_id INTEGER NOT NULL,
                text TEXT NOT NULL
            );
            CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT
            );
            CREATE TABLE system (version TEXT NOT NULL, date TEXT NOT NULL);
            CREATE TABLE tickets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                project_id INTEGER REFERENCES projects(id)
            );
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL,
                password TEXT NOT NULL
            );
            INSERT INTO system (version, date) VALUES ('0.1.0', '12 July 2026');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    async fn insert_user_and_client(pool: &SqlitePool) {
        let hash = hash_password("secret-pass").unwrap();
        sqlx::query(
            r#"INSERT INTO users ("email", "password", "username", "type", "role", "name")
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind("eyan@local")
        .bind(hash)
        .bind("eyan")
        .bind("Human")
        .bind("Owner")
        .bind("Eyan")
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(r#"INSERT INTO oauth_clients ("client_id", "name") VALUES ($1, $2)"#)
            .bind("lyra-ios")
            .bind("Lyra iOS")
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn pkce_login_issues_jwt_and_current_user() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
        }
        let pool = setup_pool().await;
        insert_user_and_client(&pool).await;

        let verifier = "pkce-verifier-value-that-is-long-enough";
        let challenge = pkce_s256(verifier);
        let auth = oauth_authorize(
            State(Arc::new(pool.clone())),
            Json(AuthorizeRequest {
                username: "eyan".into(),
                password: "secret-pass".into(),
                client_id: "lyra-ios".into(),
                code_challenge: challenge,
                code_challenge_method: "S256".into(),
            }),
        )
        .await
        .unwrap();

        let token = oauth_token(
            State(Arc::new(pool.clone())),
            Json(TokenRequest {
                grant_type: "authorization_code".into(),
                client_id: Some("lyra-ios".into()),
                code: Some(auth.code.clone()),
                code_verifier: Some(verifier.into()),
                refresh_token: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(token.token_type, "Bearer");
        assert!(!token.access_token.is_empty());

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", token.access_token).parse().unwrap(),
        );
        let user = current_user(State(Arc::new(pool.clone())), headers)
            .await
            .unwrap();
        assert_eq!(user.username, "eyan");
        assert_eq!(user.user_type, "Human");
        assert_eq!(user.name, "Eyan");
    }

    #[tokio::test]
    async fn wrong_password_is_rejected() {
        let pool = setup_pool().await;
        insert_user_and_client(&pool).await;
        let result = oauth_authorize(
            State(Arc::new(pool)),
            Json(AuthorizeRequest {
                username: "eyan".into(),
                password: "nope".into(),
                client_id: "lyra-ios".into(),
                code_challenge: "abc".into(),
                code_challenge_method: "S256".into(),
            }),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn zero_expires_at_is_invalid_grant() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
        }
        let pool = setup_pool().await;
        insert_user_and_client(&pool).await;
        sqlx::query(
            r#"INSERT INTO oauth_codes ("client_id", "code", "code_challenge", "username", "expires_at")
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind("lyra-ios")
        .bind("dead-code")
        .bind("challenge")
        .bind("eyan")
        .bind("0")
        .execute(&pool)
        .await
        .unwrap();

        let result = oauth_token(
            State(Arc::new(pool)),
            Json(TokenRequest {
                grant_type: "authorization_code".into(),
                client_id: Some("lyra-ios".into()),
                code: Some("dead-code".into()),
                code_verifier: Some("unused".into()),
                refresh_token: None,
            }),
        )
        .await;
        let err = result.err().unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.0.error, "invalid_grant");
    }

    #[tokio::test]
    async fn reused_code_is_invalid_grant() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
        }
        let pool = setup_pool().await;
        insert_user_and_client(&pool).await;
        let verifier = "pkce-verifier-value-that-is-long-enough";
        let challenge = pkce_s256(verifier);
        let auth = oauth_authorize(
            State(Arc::new(pool.clone())),
            Json(AuthorizeRequest {
                username: "eyan".into(),
                password: "secret-pass".into(),
                client_id: "lyra-ios".into(),
                code_challenge: challenge,
                code_challenge_method: "S256".into(),
            }),
        )
        .await
        .unwrap();

        let req = TokenRequest {
            grant_type: "authorization_code".into(),
            client_id: Some("lyra-ios".into()),
            code: Some(auth.code.clone()),
            code_verifier: Some(verifier.into()),
            refresh_token: None,
        };
        let _ = oauth_token(State(Arc::new(pool.clone())), Json(req.clone()))
            .await
            .unwrap();
        let second = oauth_token(State(Arc::new(pool)), Json(req)).await;
        let err = second.err().unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.0.error, "invalid_grant");
    }
}
