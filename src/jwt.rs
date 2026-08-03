use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Claim {
    // the payload for the JWT
    sub: String,
    exp: usize,
    iat: usize,    //issues at
    role: String,  // user or admin
    email: String, //users email
}
