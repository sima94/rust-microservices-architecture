use sqlx::PgPool;
use crate::models::{AuthUser, NewAuthUser};

pub struct AuthUserRepository;

impl AuthUserRepository {
    pub async fn create(pool: &PgPool, new_user: NewAuthUser) -> Result<AuthUser, sqlx::Error> {
        sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(&new_user.email)
            .bind(&new_user.password_hash)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_email(pool: &PgPool, user_email: &str) -> Result<AuthUser, sqlx::Error> {
        sqlx::query_as::<_, AuthUser>(
            "SELECT id, email, password_hash, created_at FROM auth_users WHERE email = $1"
        )
            .bind(user_email)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_id(pool: &PgPool, user_id_val: i32) -> Result<AuthUser, sqlx::Error> {
        sqlx::query_as::<_, AuthUser>(
            "SELECT id, email, password_hash, created_at FROM auth_users WHERE id = $1"
        )
            .bind(user_id_val)
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
    async fn test_create_and_find_by_email() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let unique_email = format!("test_{}@example.com", uuid::Uuid::new_v4());
        let new = NewAuthUser {
            email: unique_email.clone(),
            password_hash: "hashed_password_123".into(),
        };
        let created = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind(&new.email)
            .bind(&new.password_hash)
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(created.email, unique_email);
        assert_eq!(created.password_hash, "hashed_password_123");

        let found = sqlx::query_as::<_, AuthUser>(
            "SELECT id, email, password_hash, created_at FROM auth_users WHERE email = $1"
        )
            .bind(&unique_email)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.email, unique_email);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_find_by_id() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let created = sqlx::query_as::<_, AuthUser>(
            "INSERT INTO auth_users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, created_at"
        )
            .bind("findbyid@example.com")
            .bind("hash")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        let found = sqlx::query_as::<_, AuthUser>(
            "SELECT id, email, password_hash, created_at FROM auth_users WHERE id = $1"
        )
            .bind(created.id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(found.email, "findbyid@example.com");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_duplicate_email_fails() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let unique_email = format!("dup_{}@example.com", uuid::Uuid::new_v4());

        sqlx::query("INSERT INTO auth_users (email, password_hash) VALUES ($1, $2)")
            .bind(&unique_email)
            .bind("hash1")
            .execute(&mut *tx)
            .await
            .unwrap();

        let result = sqlx::query("INSERT INTO auth_users (email, password_hash) VALUES ($1, $2)")
            .bind(&unique_email)
            .bind("hash2")
            .execute(&mut *tx)
            .await;
        assert!(result.is_err());

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_find_nonexistent_email() {
        let pool = get_test_pool().await;
        let result = sqlx::query_as::<_, AuthUser>(
            "SELECT id, email, password_hash, created_at FROM auth_users WHERE email = $1"
        )
            .bind("nonexistent@example.com")
            .fetch_one(&pool)
            .await;
        assert!(result.is_err());
    }
}
