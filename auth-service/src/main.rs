use actix_web::{App, HttpServer, web};
use actix_web_prom::PrometheusMetricsBuilder;
use auth_service::models::dto::*;
use auth_service::{cache, config::AppConfig, controllers, db, services::kafka_service};
use dotenvy::dotenv;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Auth Service API", version = "1.0.0", description = "Authentication and OAuth2 microservice"),
    paths(
        controllers::v1::auth_controller::register,
        controllers::v1::client_controller::register_client,
        controllers::v1::oauth_controller::authorize,
        controllers::v1::oauth_controller::token,
        controllers::v1::oauth_controller::revoke,
        controllers::health_controller::health_check,
    ),
    components(schemas(
        RegisterRequest, RegisterResponse,
        ClientRegisterRequest, ClientRegisterResponse,
        LoginRequest, AuthorizeParams, AuthorizeResponse,
        TokenRequest, TokenResponse, RevokeRequest
    )),
    tags(
        (name = "Auth", description = "User registration"),
        (name = "Clients", description = "OAuth client management"),
        (name = "OAuth", description = "OAuth2 authorization flow (PKCE)"),
        (name = "Health", description = "Health check")
    )
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let ip = "0.0.0.0";
    let server_port = 8081;
    let server_address = format!("http://{ip}:{server_port}");

    println!("Starting auth-service at {server_address}");

    let db_pools = db::init_pools().await;
    let redis_pool = cache::init_redis().await;
    println!("Redis connected");

    let app_config = AppConfig::from_env();

    let kafka_broker =
        std::env::var("KAFKA_BROKER").unwrap_or_else(|_| "localhost:9092".to_string());
    println!("Connecting to Kafka broker at {kafka_broker}");
    let kafka_producer = kafka_service::create_producer(&kafka_broker);

    let prometheus = PrometheusMetricsBuilder::new("auth_service")
        .endpoint("/metrics")
        .build()
        .expect("Failed to create prometheus middleware");

    HttpServer::new(move || {
        App::new()
            .wrap(prometheus.clone())
            .app_data(web::Data::new(db_pools.clone()))
            .app_data(web::Data::new(redis_pool.clone()))
            .app_data(web::Data::new(AppConfig {
                jwt_secret: app_config.jwt_secret.clone(),
                access_token_ttl_secs: app_config.access_token_ttl_secs,
                refresh_token_ttl_days: app_config.refresh_token_ttl_days,
            }))
            .app_data(web::Data::new(kafka_producer.clone()))
            .configure(controllers::health_controller::init_routes)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
            .service(
                web::scope("/api/v1")
                    .configure(controllers::v1::auth_controller::init_routes)
                    .configure(controllers::v1::client_controller::init_routes)
                    .configure(controllers::v1::oauth_controller::init_routes),
            )
    })
    .bind((ip, server_port))?
    .run()
    .await
}
