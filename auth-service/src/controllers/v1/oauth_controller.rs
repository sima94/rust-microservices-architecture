use crate::cache::RedisPool;
use crate::config::AppConfig;
use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::dto::*;
use crate::services::oauth_service::OAuthService;
use actix_web::{HttpResponse, web};

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/oauth")
            .route("/authorize", web::post().to(authorize))
            .route("/token", web::post().to(token))
            .route("/revoke", web::post().to(revoke)),
    );
}

#[utoipa::path(
    post, path = "/api/v1/oauth/authorize",
    tag = "OAuth",
    params(AuthorizeParams),
    request_body = LoginRequest,
    responses(
        (status = 200, body = AuthorizeResponse, description = "Authorization code issued"),
        (status = 401, description = "Invalid credentials"),
        (status = 400, description = "Invalid client or request")
    )
)]
pub async fn authorize(
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    params: web::Query<AuthorizeParams>,
    json: web::Json<LoginRequest>,
) -> Result<HttpResponse, ServiceError> {
    let response =
        OAuthService::authorize(pools, redis, params.into_inner(), json.into_inner()).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[utoipa::path(
    post, path = "/api/v1/oauth/token",
    tag = "OAuth",
    request_body = TokenRequest,
    responses(
        (status = 200, body = TokenResponse, description = "Token issued"),
        (status = 400, description = "Invalid grant"),
        (status = 401, description = "Invalid client")
    )
)]
pub async fn token(
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    config: web::Data<AppConfig>,
    form: web::Form<TokenRequest>,
) -> Result<HttpResponse, ServiceError> {
    let response = OAuthService::token(pools, redis, config, form.into_inner()).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[utoipa::path(
    post, path = "/api/v1/oauth/revoke",
    tag = "OAuth",
    request_body = RevokeRequest,
    responses(
        (status = 200, description = "Token revoked")
    )
)]
pub async fn revoke(
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    form: web::Form<RevokeRequest>,
) -> Result<HttpResponse, ServiceError> {
    OAuthService::revoke(pools, redis, form.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}
