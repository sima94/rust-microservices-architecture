use sqlx::PgPool;
use crate::models::refresh_token::{RefreshToken, NewRefreshToken};

pub struct RefreshTokenRepository;

impl RefreshTokenRepository {
    pub async fn create(pool: &PgPool, new_token: NewRefreshToken) -> Result<RefreshToken, sqlx::Error> {
        sqlx::query_as::<_, RefreshToken>(
            "INSERT INTO refresh_tokens (token, client_id, user_id, scopes, expires_at) VALUES ($1, $2, $3, $4, $5) RETURNING id, token, client_id, user_id, scopes, expires_at, created_at"
        )
            .bind(&new_token.token)
            .bind(&new_token.client_id)
            .bind(new_token.user_id)
            .bind(&new_token.scopes)
            .bind(new_token.expires_at)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_token(pool: &PgPool, token_val: &str) -> Result<RefreshToken, sqlx::Error> {
        sqlx::query_as::<_, RefreshToken>(
            "SELECT id, token, client_id, user_id, scopes, expires_at, created_at FROM refresh_tokens WHERE token = $1"
        )
            .bind(token_val)
            .fetch_one(pool)
            .await
    }

    pub async fn delete_by_token(pool: &PgPool, token_val: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE token = $1")
            .bind(token_val)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_by_user_id(pool: &PgPool, uid: i32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
            .bind(uid)
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
    async fn test_create_and_find_token() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let user = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(format!("tokentest_{}@example.com", uuid::Uuid::new_v4()))
            .bind("hash")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        let unique_token = format!("refresh-token-{}", uuid::Uuid::new_v4());
        let created = sqlx::query_as::<_, RefreshToken>(
            "INSERT INTO refresh_tokens (token, client_id, user_id, scopes, expires_at) VALUES ($1, $2, $3, $4, $5) RETURNING id, token, client_id, user_id, scopes, expires_at, created_at"
        )
            .bind(&unique_token)
            .bind("client-1")
            .bind(user.id)
            .bind("read write")
            .bind(Utc::now().naive_utc() + chrono::Duration::days(7))
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(created.token, unique_token);
        assert_eq!(created.user_id, user.id);

        let found = sqlx::query_as::<_, RefreshToken>(
            "SELECT id, token, client_id, user_id, scopes, expires_at, created_at FROM refresh_tokens WHERE token = $1"
        )
            .bind(&unique_token)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.scopes, "read write");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_by_token() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let user = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(format!("deltoken_{}@example.com", uuid::Uuid::new_v4()))
            .bind("hash")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        let unique_token = format!("token-to-delete-{}", uuid::Uuid::new_v4());
        sqlx::query(
            "INSERT INTO refresh_tokens (token, client_id, user_id, scopes, expires_at) VALUES ($1, $2, $3, $4, $5)"
        )
            .bind(&unique_token)
            .bind("client-1")
            .bind(user.id)
            .bind("read")
            .bind(Utc::now().naive_utc() + chrono::Duration::days(7))
            .execute(&mut *tx)
            .await
            .unwrap();

        let result = sqlx::query("DELETE FROM refresh_tokens WHERE token = $1")
            .bind(&unique_token)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);

        let found = sqlx::query_as::<_, RefreshToken>(
            "SELECT id, token, client_id, user_id, scopes, expires_at, created_at FROM refresh_tokens WHERE token = $1"
        )
            .bind(&unique_token)
            .fetch_optional(&mut *tx)
            .await
            .unwrap();
        assert!(found.is_none());

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_by_user_id() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let user = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(format!("deluserid_{}@example.com", uuid::Uuid::new_v4()))
            .bind("hash")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO refresh_tokens (token, client_id, user_id, scopes, expires_at) VALUES ($1, $2, $3, $4, $5)"
        )
            .bind(format!("user-token-1-{}", uuid::Uuid::new_v4()))
            .bind("client-1")
            .bind(user.id)
            .bind("read")
            .bind(Utc::now().naive_utc() + chrono::Duration::days(7))
            .execute(&mut *tx)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO refresh_tokens (token, client_id, user_id, scopes, expires_at) VALUES ($1, $2, $3, $4, $5)"
        )
            .bind(format!("user-token-2-{}", uuid::Uuid::new_v4()))
            .bind("client-1")
            .bind(user.id)
            .bind("read")
            .bind(Utc::now().naive_utc() + chrono::Duration::days(7))
            .execute(&mut *tx)
            .await
            .unwrap();

        let result = sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
            .bind(user.id)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 2);

        tx.rollback().await.unwrap();
    }
}
