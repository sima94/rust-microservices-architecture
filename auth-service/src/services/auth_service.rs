use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::dto::{RegisterRequest, RegisterResponse};
use crate::models::{AuthUser, NewAuthUser};
use crate::repositories::auth_user_repository::AuthUserRepository;
use actix_web::web;
use argon2::password_hash::{PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;

pub struct AuthService;

impl AuthService {
    pub async fn register(
        pools: web::Data<DbPools>,
        req: RegisterRequest,
    ) -> Result<RegisterResponse, ServiceError> {
        if req.password.len() < 8 {
            return Err(ServiceError::InvalidRequest(
                "Password must be at least 8 characters".into(),
            ));
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| ServiceError::InternalError(format!("Hash error: {}", e)))?
            .to_string();

        let new_user = NewAuthUser {
            email: req.email.clone(),
            password_hash: hash,
        };

        let user = AuthUserRepository::create(&pools.write, new_user)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref db_err) = e
                    && db_err.is_unique_violation()
                {
                    return ServiceError::Conflict("Email already registered".into());
                }
                ServiceError::from(e)
            })?;

        Ok(RegisterResponse {
            id: user.id,
            email: user.email,
        })
    }

    pub async fn verify_credentials(
        pools: &DbPools,
        user_email: &str,
        password: &str,
    ) -> Result<AuthUser, ServiceError> {
        let user = AuthUserRepository::find_by_email(&pools.read, user_email)
            .await
            .map_err(|_| ServiceError::Unauthorized("Invalid credentials".into()))?;

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| ServiceError::InternalError(format!("Hash parse error: {}", e)))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| ServiceError::Unauthorized("Invalid credentials".into()))?;

        Ok(user)
    }
}
