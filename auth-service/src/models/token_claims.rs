use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub client_id: String,
    pub scopes: String,
    pub exp: usize,
    pub iat: usize,
}
