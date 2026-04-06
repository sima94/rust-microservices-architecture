use actix_web::{web, HttpResponse};
use crate::cache::RedisPool;
use crate::db::DbPools;

pub async fn health_check(
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    service_name: &str,
) -> HttpResponse {
    let write_ok = pools.write.acquire().await.is_ok();
    let read_ok = pools.read.acquire().await.is_ok();
    let redis_ok = {
        let mut conn = redis.as_ref().clone();
        let result: Result<String, _> = redis::cmd("PING").query_async(&mut conn).await;
        result.is_ok()
    };

    if write_ok && read_ok && redis_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "healthy",
            "service": service_name,
            "master": "up",
            "replica": "up",
            "redis": "up"
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "degraded",
            "service": service_name,
            "master": if write_ok { "up" } else { "down" },
            "replica": if read_ok { "up" } else { "down" },
            "redis": if redis_ok { "up" } else { "down" }
        }))
    }
}
