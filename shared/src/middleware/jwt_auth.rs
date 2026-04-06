use actix_web::{web, HttpRequest, FromRequest, dev::Payload};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::future::{Ready, ready};
use crate::errors::ServiceError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub client_id: String,
    pub scopes: String,
    pub exp: usize,
    pub iat: usize,
}

pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: Option<String>,
    pub client_id: String,
    pub scopes: Vec<String>,
}

impl FromRequest for AuthenticatedUser {
    type Error = ServiceError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(extract_user(req))
    }
}

fn extract_user(req: &HttpRequest) -> Result<AuthenticatedUser, ServiceError> {
    let jwt_secret = req.app_data::<web::Data<String>>()
        .ok_or_else(|| ServiceError::InternalError("JWT secret not configured".into()))?;

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ServiceError::Unauthorized("Missing Authorization header".into()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ServiceError::Unauthorized("Invalid Authorization format. Use: Bearer <token>".into()))?;

    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| ServiceError::Unauthorized(format!("Invalid token: {}", e)))?
    .claims;

    Ok(AuthenticatedUser {
        user_id: claims.sub,
        email: claims.email,
        client_id: claims.client_id,
        scopes: claims.scopes.split_whitespace().map(|s| s.to_string()).collect(),
    })
}
