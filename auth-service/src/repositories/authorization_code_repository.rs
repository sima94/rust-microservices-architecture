use sqlx::PgPool;
use crate::models::authorization_code::{AuthorizationCode, NewAuthorizationCode};

pub struct AuthorizationCodeRepository;

impl AuthorizationCodeRepository {
    pub async fn create(pool: &PgPool, new_code: NewAuthorizationCode) -> Result<AuthorizationCode, sqlx::Error> {
        sqlx::query_as::<_, AuthorizationCode>(
            "INSERT INTO authorization_codes (code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id, code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at, used, created_at"
        )
            .bind(&new_code.code)
            .bind(&new_code.client_id)
            .bind(new_code.user_id)
            .bind(&new_code.redirect_uri)
            .bind(&new_code.scopes)
            .bind(&new_code.code_challenge)
            .bind(&new_code.code_challenge_method)
            .bind(new_code.expires_at)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_code(pool: &PgPool, code_val: &str) -> Result<AuthorizationCode, sqlx::Error> {
        sqlx::query_as::<_, AuthorizationCode>(
            "SELECT id, code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at, used, created_at FROM authorization_codes WHERE code = $1"
        )
            .bind(code_val)
            .fetch_one(pool)
            .await
    }

    pub async fn mark_used(pool: &PgPool, code_val: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE authorization_codes SET used = true WHERE code = $1")
            .bind(code_val)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::models::AuthUser;

    async fn get_test_pool() -> PgPool {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for tests");
        sqlx::PgPool::connect(&url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    async fn test_create_and_find_code() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let user = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(format!("codetest_{}@example.com", uuid::Uuid::new_v4()))
            .bind("hash")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        let unique_code = format!("test-auth-code-{}", uuid::Uuid::new_v4());
        let created = sqlx::query_as::<_, AuthorizationCode>(
            "INSERT INTO authorization_codes (code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id, code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at, used, created_at"
        )
            .bind(&unique_code)
            .bind("client-1")
            .bind(user.id)
            .bind("http://localhost/callback")
            .bind("read")
            .bind("challenge123")
            .bind("S256")
            .bind(Utc::now().naive_utc() + chrono::Duration::minutes(10))
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(created.code, unique_code);
        assert!(!created.used);

        let found = sqlx::query_as::<_, AuthorizationCode>(
            "SELECT id, code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at, used, created_at FROM authorization_codes WHERE code = $1"
        )
            .bind(&unique_code)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.user_id, user.id);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_mark_used() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let user = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(format!("markused_{}@example.com", uuid::Uuid::new_v4()))
            .bind("hash")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        let unique_code = format!("to-be-used-{}", uuid::Uuid::new_v4());
        sqlx::query(
            "INSERT INTO authorization_codes (code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
            .bind(&unique_code)
            .bind("client-1")
            .bind(user.id)
            .bind("http://localhost/callback")
            .bind("read")
            .bind("challenge")
            .bind("S256")
            .bind(Utc::now().naive_utc() + chrono::Duration::minutes(10))
            .execute(&mut *tx)
            .await
            .unwrap();

        let result = sqlx::query("UPDATE authorization_codes SET used = true WHERE code = $1")
            .bind(&unique_code)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);

        let found = sqlx::query_as::<_, AuthorizationCode>(
            "SELECT id, code, client_id, user_id, redirect_uri, scopes, code_challenge, code_challenge_method, expires_at, used, created_at FROM authorization_codes WHERE code = $1"
        )
            .bind(&unique_code)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert!(found.used);

        tx.rollback().await.unwrap();
    }
}
