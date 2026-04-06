use crate::cache::{self, RedisPool};
use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::{NewUser, UpdateUser, User};
use crate::repositories::user_repository::UserRepository;
use actix_web::web;

pub struct UserService;

impl UserService {
    pub async fn create_user(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        user: NewUser,
    ) -> Result<User, ServiceError> {
        let created = UserRepository::create(&pools.write, user)
            .await
            .map_err(ServiceError::from)?;

        // Cache the new user + invalidate list cache
        cache::set_cached(&redis, &cache::user_cache_key(created.id), &created).await;
        cache::invalidate(&redis, &cache::users_list_cache_key()).await;

        Ok(created)
    }

    pub async fn get_user_by_id(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        user_id_val: i32,
    ) -> Result<User, ServiceError> {
        let cache_key = cache::user_cache_key(user_id_val);

        // Try cache first
        if let Some(user) = cache::get_cached::<User>(&redis, &cache_key).await {
            println!("Cache HIT for user:{}", user_id_val);
            return Ok(user);
        }

        println!("Cache MISS for user:{}", user_id_val);

        // Cache miss - query read replica
        let user = UserRepository::find_by_id(&pools.read, user_id_val)
            .await
            .map_err(ServiceError::from)?;

        // Store in cache
        cache::set_cached(&redis, &cache_key, &user).await;

        Ok(user)
    }

    pub async fn list_users(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
    ) -> Result<Vec<User>, ServiceError> {
        let cache_key = cache::users_list_cache_key();

        // Try cache first
        if let Some(users) = cache::get_cached::<Vec<User>>(&redis, &cache_key).await {
            println!("Cache HIT for users:list");
            return Ok(users);
        }

        println!("Cache MISS for users:list");

        let users = UserRepository::find_all(&pools.read)
            .await
            .map_err(ServiceError::from)?;

        // Cache the list (shorter TTL since it changes more often)
        cache::set_cached(&redis, &cache_key, &users).await;

        Ok(users)
    }

    pub async fn update_user(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        user_id_val: i32,
        update: UpdateUser,
    ) -> Result<User, ServiceError> {
        let updated = UserRepository::update(&pools.write, user_id_val, update)
            .await
            .map_err(ServiceError::from)?;

        // Update cache with fresh data + invalidate list
        cache::set_cached(&redis, &cache::user_cache_key(user_id_val), &updated).await;
        cache::invalidate(&redis, &cache::users_list_cache_key()).await;

        Ok(updated)
    }

    pub async fn delete_user(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        user_id_val: i32,
    ) -> Result<(), ServiceError> {
        let count = UserRepository::delete(&pools.write, user_id_val)
            .await
            .map_err(ServiceError::from)?;
        if count == 0 {
            return Err(ServiceError::NotFound);
        }

        // Invalidate cache
        cache::invalidate(&redis, &cache::user_cache_key(user_id_val)).await;
        cache::invalidate(&redis, &cache::users_list_cache_key()).await;

        Ok(())
    }
}
