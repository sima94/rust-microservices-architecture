use crate::errors::ServiceError;
use crate::middleware::jwt_auth::AuthenticatedUser;

pub fn require_scope(auth: &AuthenticatedUser, required: &str) -> Result<(), ServiceError> {
    if auth.scopes.iter().any(|s| s == required) {
        Ok(())
    } else {
        Err(ServiceError::Forbidden(format!("Insufficient scope. Required: {}", required)))
    }
}
