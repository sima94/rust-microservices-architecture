use actix_web::{web, HttpResponse};
use crate::cache::RedisPool;
use crate::db::DbPools;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health_check));
}

#[utoipa::path(
    get, path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service healthy"),
        (status = 503, description = "Service degraded")
    )
)]
pub async fn health_check(
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
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
            "service": "user-service",
            "master": "up",
            "replica": "up",
            "redis": "up"
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "degraded",
            "service": "user-service",
            "master": if write_ok { "up" } else { "down" },
            "replica": if read_ok { "up" } else { "down" },
            "redis": if redis_ok { "up" } else { "down" }
        }))
    }
}
