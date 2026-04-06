use crate::models::Claims;
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};

pub struct TokenService;

impl TokenService {
    pub fn create_access_token(
        user_id: &str,
        email: Option<&str>,
        client_id: &str,
        scopes: &str,
        secret: &str,
        ttl_secs: i64,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.map(|e| e.to_string()),
            client_id: client_id.to_string(),
            scopes: scopes.to_string(),
            exp: now + ttl_secs as usize,
            iat: now,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
    }

    pub fn validate_token(
        token: &str,
        secret: &str,
    ) -> Result<Claims, jsonwebtoken::errors::Error> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(token_data.claims)
    }

    pub fn generate_refresh_token() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-jwt";

    #[test]
    fn test_create_and_validate_token() {
        let token = TokenService::create_access_token(
            "123",
            Some("test@example.com"),
            "client-1",
            "read write",
            TEST_SECRET,
            300,
        )
        .unwrap();

        let claims = TokenService::validate_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "123");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
        assert_eq!(claims.client_id, "client-1");
        assert_eq!(claims.scopes, "read write");
    }

    #[test]
    fn test_validate_with_wrong_secret_fails() {
        let token =
            TokenService::create_access_token("123", None, "client-1", "read", TEST_SECRET, 300)
                .unwrap();

        let result = TokenService::validate_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token_fails() {
        // Kreiraj token sa exp u proslosti (exp = 0 = 1970-01-01)
        let claims = crate::models::Claims {
            sub: "123".to_string(),
            email: None,
            client_id: "client-1".to_string(),
            scopes: "read".to_string(),
            exp: 0,
            iat: 0,
        };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = TokenService::validate_token(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_refresh_token_is_unique() {
        let t1 = TokenService::generate_refresh_token();
        let t2 = TokenService::generate_refresh_token();
        assert_ne!(t1, t2);
        assert_eq!(t1.len(), 36); // UUID v4 format
    }
}
