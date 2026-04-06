use serde::{Deserialize, Serialize};
use utoipa::{ToSchema, IntoParams};

#[derive(Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct RegisterResponse {
    pub id: i32,
    pub email: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ClientRegisterRequest {
    pub client_name: String,
    pub redirect_uri: String,
    pub scopes: String,
}

#[derive(Serialize, ToSchema)]
pub struct ClientRegisterResponse {
    pub client_id: String,
    pub client_secret: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub scopes: String,
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct AuthorizeParams {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

#[derive(Serialize, ToSchema)]
pub struct AuthorizeResponse {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize, ToSchema)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: String,
    pub client_secret: String,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RevokeRequest {
    pub token: String,
    pub client_id: String,
    pub client_secret: String,
}
