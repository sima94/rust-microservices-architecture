use sqlx::PgPool;
use crate::models::{OAuthClient, NewOAuthClient};

pub struct OAuthClientRepository;

impl OAuthClientRepository {
    pub async fn create(pool: &PgPool, new_client: NewOAuthClient) -> Result<OAuthClient, sqlx::Error> {
        sqlx::query_as::<_, OAuthClient>(
            "INSERT INTO oauth_clients (client_id, client_secret_hash, client_name, redirect_uri, scopes) VALUES ($1, $2, $3, $4, $5) RETURNING id, client_id, client_secret_hash, client_name, redirect_uri, scopes, created_at"
        )
            .bind(&new_client.client_id)
            .bind(&new_client.client_secret_hash)
            .bind(&new_client.client_name)
            .bind(&new_client.redirect_uri)
            .bind(&new_client.scopes)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_client_id(pool: &PgPool, cid: &str) -> Result<OAuthClient, sqlx::Error> {
        sqlx::query_as::<_, OAuthClient>(
            "SELECT id, client_id, client_secret_hash, client_name, redirect_uri, scopes, created_at FROM oauth_clients WHERE client_id = $1"
        )
            .bind(cid)
            .fetch_one(pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get_test_pool() -> PgPool {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for tests");
        sqlx::PgPool::connect(&url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    async fn test_create_and_find_client() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let unique_cid = format!("test-client-{}", uuid::Uuid::new_v4());
        let created = sqlx::query_as::<_, OAuthClient>(
            "INSERT INTO oauth_clients (client_id, client_secret_hash, client_name, redirect_uri, scopes) VALUES ($1, $2, $3, $4, $5) RETURNING id, client_id, client_secret_hash, client_name, redirect_uri, scopes, created_at"
        )
            .bind(&unique_cid)
            .bind("hashed_secret")
            .bind("Test App")
            .bind("http://localhost:3000/callback")
            .bind("read write")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(created.client_id, unique_cid);
        assert_eq!(created.client_name, "Test App");
        assert_eq!(created.scopes, "read write");

        let found = sqlx::query_as::<_, OAuthClient>(
            "SELECT id, client_id, client_secret_hash, client_name, redirect_uri, scopes, created_at FROM oauth_clients WHERE client_id = $1"
        )
            .bind(&unique_cid)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.redirect_uri, "http://localhost:3000/callback");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_find_nonexistent_client() {
        let pool = get_test_pool().await;
        let result = sqlx::query_as::<_, OAuthClient>(
            "SELECT id, client_id, client_secret_hash, client_name, redirect_uri, scopes, created_at FROM oauth_clients WHERE client_id = $1"
        )
            .bind("nonexistent")
            .fetch_one(&pool)
            .await;
        assert!(result.is_err());
    }
}
