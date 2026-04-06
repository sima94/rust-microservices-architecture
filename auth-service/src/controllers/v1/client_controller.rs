use crate::cache::RedisPool;
use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::dto::{ClientRegisterRequest, ClientRegisterResponse};
use crate::services::oauth_service::OAuthService;
use actix_web::{HttpResponse, web};

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/clients").route("/register", web::post().to(register_client)));
}

#[utoipa::path(
    post, path = "/api/v1/clients/register",
    tag = "Clients",
    request_body = ClientRegisterRequest,
    responses(
        (status = 201, body = ClientRegisterResponse, description = "OAuth client registered")
    )
)]
pub async fn register_client(
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    json: web::Json<ClientRegisterRequest>,
) -> Result<HttpResponse, ServiceError> {
    let response = OAuthService::register_client(pools, redis, json.into_inner()).await?;
    Ok(HttpResponse::Created().json(response))
}
