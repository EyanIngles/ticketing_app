use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const ACCESS_TTL_SECS: u64 = 3600;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Claim {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub username: String,
    #[serde(rename = "type")]
    pub user_type: String,
    pub role: String,
    pub name: String,
}

pub fn encode_access_token(
    secret: &str,
    username: &str,
    user_type: &str,
    role: &str,
    name: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = now_secs();
    let claim = Claim {
        sub: username.to_string(),
        iat: now as usize,
        exp: (now + ACCESS_TTL_SECS) as usize,
        username: username.to_string(),
        user_type: user_type.to_string(),
        role: role.to_string(),
        name: name.to_string(),
    };
    encode(
        &Header::default(),
        &claim,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn decode_access_token(
    secret: &str,
    token: &str,
) -> Result<Claim, jsonwebtoken::errors::Error> {
    let data = decode::<Claim>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
