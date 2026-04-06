use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use serde::{Serialize, de::DeserializeOwned};

pub type RedisPool = ConnectionManager;

const CACHE_TTL: u64 = 600; // 10 minutes (clients change rarely)

pub async fn init_redis() -> RedisPool {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let client = redis::Client::open(redis_url.as_str()).expect("Failed to create Redis client");

    ConnectionManager::new(client)
        .await
        .expect("Failed to connect to Redis")
}

pub async fn get_cached<T: DeserializeOwned>(redis: &RedisPool, key: &str) -> Option<T> {
    let mut conn = redis.clone();
    let result: redis::RedisResult<String> = conn.get(key.to_string()).await;
    match result {
        Ok(json) => serde_json::from_str(&json).ok(),
        Err(_) => None,
    }
}

pub async fn set_cached<T: Serialize>(redis: &RedisPool, key: &str, value: &T) {
    let mut conn = redis.clone();
    if let Ok(json) = serde_json::to_string(value) {
        let _: redis::RedisResult<()> = conn.set_ex(key.to_string(), json, CACHE_TTL).await;
    }
}

pub async fn invalidate(redis: &RedisPool, key: &str) {
    let mut conn = redis.clone();
    let _: redis::RedisResult<()> = conn.del(key.to_string()).await;
}

pub fn oauth_client_cache_key(client_id: &str) -> String {
    format!("oauth_client:{}", client_id)
}
