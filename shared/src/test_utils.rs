use jsonwebtoken::{encode, Header, EncodingKey};
use serde::Serialize;
use crate::db::{DbPool, DbPools};
use sqlx::postgres::PgPoolOptions;

pub const TEST_JWT_SECRET: &str = "test-secret-for-integration-tests";

pub async fn get_test_pool() -> DbPool {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for tests");

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create test pool")
}

pub async fn get_test_pools() -> DbPools {
    let pool = get_test_pool().await;
    DbPools {
        write: pool.clone(),
        read: pool,
    }
}

pub fn create_test_token(scopes: &str) -> String {
    create_test_token_with_secret(scopes, TEST_JWT_SECRET)
}

pub fn create_test_token_with_secret(scopes: &str, secret: &str) -> String {
    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        email: Option<String>,
        client_id: String,
        scopes: String,
        exp: usize,
        iat: usize,
    }

    let now = chrono::Utc::now().timestamp() as usize;
    let claims = TestClaims {
        sub: "test-user-1".into(),
        email: Some("test@example.com".into()),
        client_id: "test-client".into(),
        scopes: scopes.into(),
        exp: now + 3600,
        iat: now,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .unwrap()
}
