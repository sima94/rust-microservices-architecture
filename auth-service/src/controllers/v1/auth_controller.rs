use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::dto::{RegisterRequest, RegisterResponse};
use crate::services::auth_service::AuthService;
use crate::services::kafka_service::{UserEvent, publish_user_event};
use actix_web::{HttpResponse, web};
use rdkafka::producer::FutureProducer;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/auth").route("/register", web::post().to(register)));
}

#[utoipa::path(
    post, path = "/api/v1/auth/register",
    tag = "Auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, body = RegisterResponse, description = "User registered"),
        (status = 409, description = "Email already exists"),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn register(
    pools: web::Data<DbPools>,
    producer: web::Data<FutureProducer>,
    json: web::Json<RegisterRequest>,
) -> Result<HttpResponse, ServiceError> {
    let response = AuthService::register(pools, json.into_inner()).await?;

    let event = UserEvent {
        event_type: "user.registered".to_string(),
        user_id: response.id,
        email: response.email.clone(),
    };
    if let Err(e) = publish_user_event(&producer, "user.events", &event).await {
        eprintln!("Failed to publish Kafka event: {}", e);
    }

    Ok(HttpResponse::Created().json(response))
}
