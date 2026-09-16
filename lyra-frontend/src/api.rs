use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use gloo_storage::{LocalStorage, Storage};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

pub const JWT_KEY: &str = "JWT";

#[derive(Clone, Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct Comment {
    pub id: i64,
    pub text: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_type: String,
    #[serde(default)]
    pub author_role: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub display: String,
}

#[derive(Clone, Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct Ticket {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub project_id: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub github_pr_url: String,
    #[serde(default)]
    pub last_model: String,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

#[derive(Clone, Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct Project {
    pub id: i32,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tickets: Vec<Ticket>,
}

#[derive(Clone, Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct PermissionRequest {
    pub id: i64,
    #[serde(default)]
    pub ticket_id: i64,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_type: String,
    #[serde(default)]
    pub author_role: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub opencode_session_id: String,
    #[serde(default)]
    pub permission_id: String,
    #[serde(default)]
    pub is_used: bool,
}

#[derive(Serialize)]
pub struct CreateTicket {
    pub name: String,
    pub description: String,
    pub project_id: i64,
}

#[derive(Serialize)]
pub struct CreateComment {
    pub text: String,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
}

pub fn api_base() -> Result<String, String> {
    match option_env!("LYRA_API_BASE") {
        Some(base) if !base.trim().is_empty() => {
            Ok(base.trim().trim_end_matches('/').to_string())
        }
        _ => Err(
            "LYRA_API_BASE is not set. Rebuild with LYRA_API_BASE=https://YOUR_PI_HOST dx serve"
                .into(),
        ),
    }
}

pub fn oauth_client_id() -> &'static str {
    option_env!("LYRA_OAUTH_CLIENT_ID").unwrap_or("lyra-ios")
}

pub fn jwt_token() -> Option<String> {
    LocalStorage::get::<String>(JWT_KEY)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn set_jwt_token(token: &str) {
    let _ = LocalStorage::set(JWT_KEY, token);
}

pub fn clear_jwt_token() {
    LocalStorage::delete(JWT_KEY);
}

pub fn pkce_s256_pair() -> Result<(String, String), String> {
    let mut bytes = [0u8; 32];
    fill_random(&mut bytes)?;
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest);
    Ok((verifier, challenge))
}

fn fill_random(buf: &mut [u8]) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let crypto = window
        .crypto()
        .map_err(|_| "crypto unavailable".to_string())?;
    crypto
        .get_random_values_with_u8_array(buf)
        .map_err(|_| "getRandomValues failed".to_string())?;
    Ok(())
}

fn apply_bearer(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match jwt_token() {
        Some(token) => builder.header("Authorization", format!("Bearer {token}")),
        None => builder,
    }
}

async fn error_from_response(response: reqwest::Response) -> String {
    let status = response.status();
    match response.json::<ErrorBody>().await {
        Ok(body) if !body.error.is_empty() => body.error,
        _ => format!("HTTP {status}"),
    }
}

pub async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{}{path}", api_base()?);
    let response = apply_bearer(reqwest::Client::new().get(url))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(error_from_response(response).await);
    }
    response.json::<T>().await.map_err(|e| e.to_string())
}

pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    let url = format!("{}{path}", api_base()?);
    let response = apply_bearer(reqwest::Client::new().post(url).json(body))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(error_from_response(response).await);
    }
    response.json::<T>().await.map_err(|e| e.to_string())
}

pub async fn post_empty<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{}{path}", api_base()?);
    let response = apply_bearer(reqwest::Client::new().post(url))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(error_from_response(response).await);
    }
    response.json::<T>().await.map_err(|e| e.to_string())
}
