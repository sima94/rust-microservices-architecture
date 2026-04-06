use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OAuthClient {
    pub id: i32,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret_hash: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub scopes: String,
    pub created_at: NaiveDateTime,
}

pub struct NewOAuthClient {
    pub client_id: String,
    pub client_secret_hash: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub scopes: String,
}
