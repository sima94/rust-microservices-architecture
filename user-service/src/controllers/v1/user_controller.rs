use actix_web::{web, HttpResponse};
use crate::cache::RedisPool;
use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::{User, NewUser, UpdateUser};
use crate::services::user_service::UserService;
use crate::middleware::jwt_auth::AuthenticatedUser;
use crate::middleware::scopes::require_scope;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .route("", web::get().to(list_users))
            .route("/{id}", web::get().to(get_user))
            .route("", web::post().to(create_user))
            .route("/{id}", web::put().to(update_user))
            .route("/{id}", web::delete().to(delete_user)),
    );
}

#[utoipa::path(
    get, path = "/api/v1/users",
    tag = "Users",
    responses(
        (status = 200, body = Vec<User>, description = "List of all users"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_users(
    auth: AuthenticatedUser,
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
) -> Result<HttpResponse, ServiceError> {
    require_scope(&auth, "read")?;
    let users = UserService::list_users(pools, redis).await?;
    Ok(HttpResponse::Ok().json(users))
}

#[utoipa::path(
    get, path = "/api/v1/users/{id}",
    tag = "Users",
    params(("id" = i32, Path, description = "User ID")),
    responses(
        (status = 200, body = User, description = "User found"),
        (status = 404, description = "User not found"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_user(
    auth: AuthenticatedUser,
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    require_scope(&auth, "read")?;
    let user = UserService::get_user_by_id(pools, redis, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(user))
}

#[utoipa::path(
    post, path = "/api/v1/users",
    tag = "Users",
    request_body = NewUser,
    responses(
        (status = 201, body = User, description = "User created"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_user(
    auth: AuthenticatedUser,
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    json: web::Json<NewUser>,
) -> Result<HttpResponse, ServiceError> {
    require_scope(&auth, "write")?;
    let user = UserService::create_user(pools, redis, json.into_inner()).await?;
    Ok(HttpResponse::Created().json(user))
}

#[utoipa::path(
    put, path = "/api/v1/users/{id}",
    tag = "Users",
    params(("id" = i32, Path, description = "User ID")),
    request_body = UpdateUser,
    responses(
        (status = 200, body = User, description = "User updated"),
        (status = 404, description = "User not found"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_user(
    auth: AuthenticatedUser,
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    path: web::Path<i32>,
    json: web::Json<UpdateUser>,
) -> Result<HttpResponse, ServiceError> {
    require_scope(&auth, "write")?;
    let user = UserService::update_user(pools, redis, path.into_inner(), json.into_inner()).await?;
    Ok(HttpResponse::Ok().json(user))
}

#[utoipa::path(
    delete, path = "/api/v1/users/{id}",
    tag = "Users",
    params(("id" = i32, Path, description = "User ID")),
    responses(
        (status = 204, description = "User deleted"),
        (status = 404, description = "User not found"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_user(
    auth: AuthenticatedUser,
    pools: web::Data<DbPools>,
    redis: web::Data<RedisPool>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    require_scope(&auth, "write")?;
    UserService::delete_user(pools, redis, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}
