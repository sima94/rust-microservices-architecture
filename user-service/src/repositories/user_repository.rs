use crate::models::{NewUser, UpdateUser, User};
use sqlx::PgPool;

pub struct UserRepository;

impl UserRepository {
    pub async fn find_by_id(pool: &PgPool, user_id_val: i32) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT id, name, email FROM users WHERE id = $1")
            .bind(user_id_val)
            .fetch_one(pool)
            .await
    }

    pub async fn find_all(pool: &PgPool) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT id, name, email FROM users")
            .fetch_all(pool)
            .await
    }

    pub async fn create(pool: &PgPool, new_user: NewUser) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email",
        )
        .bind(&new_user.name)
        .bind(&new_user.email)
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &PgPool,
        user_id_val: i32,
        changeset: UpdateUser,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "UPDATE users SET name = COALESCE($1, name), email = COALESCE($2, email) WHERE id = $3 RETURNING id, name, email"
        )
            .bind(&changeset.name)
            .bind(&changeset.email)
            .bind(user_id_val)
            .fetch_one(pool)
            .await
    }

    pub async fn delete(pool: &PgPool, user_id_val: i32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id_val)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get_test_pool() -> PgPool {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
        sqlx::PgPool::connect(&url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    async fn test_create_and_find_user() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let new = NewUser {
            name: "Test User".into(),
            email: "test_sqlx@example.com".into(),
        };
        let created = sqlx::query_as::<_, User>(
            "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email",
        )
        .bind(&new.name)
        .bind(&new.email)
        .fetch_one(&mut *tx)
        .await
        .unwrap();

        assert_eq!(created.name, "Test User");
        assert_eq!(created.email, "test_sqlx@example.com");

        let found = sqlx::query_as::<_, User>("SELECT id, name, email FROM users WHERE id = $1")
            .bind(created.id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.email, "test_sqlx@example.com");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_find_all() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
            .bind("User1")
            .bind("u1_sqlx@test.com")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
            .bind("User2")
            .bind("u2_sqlx@test.com")
            .execute(&mut *tx)
            .await
            .unwrap();

        let all = sqlx::query_as::<_, User>("SELECT id, name, email FROM users")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
        assert!(all.len() >= 2);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_update_user() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let created = sqlx::query_as::<_, User>(
            "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email",
        )
        .bind("Original")
        .bind("orig_sqlx@test.com")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

        let updated = sqlx::query_as::<_, User>(
            "UPDATE users SET name = COALESCE($1, name), email = COALESCE($2, email) WHERE id = $3 RETURNING id, name, email"
        )
            .bind(Some("Updated"))
            .bind(None::<String>)
            .bind(created.id)
            .fetch_one(&mut *tx).await.unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.email, "orig_sqlx@test.com");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_user() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();

        let created = sqlx::query_as::<_, User>(
            "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email",
        )
        .bind("ToDelete")
        .bind("del_sqlx@test.com")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(created.id)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);

        let found = sqlx::query_as::<_, User>("SELECT id, name, email FROM users WHERE id = $1")
            .bind(created.id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap();
        assert!(found.is_none());

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_find_nonexistent_returns_error() {
        let pool = get_test_pool().await;
        let result = sqlx::query_as::<_, User>("SELECT id, name, email FROM users WHERE id = $1")
            .bind(999999)
            .fetch_one(&pool)
            .await;
        assert!(result.is_err());
    }
}
