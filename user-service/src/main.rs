use actix_web::{App, HttpServer, web};
use actix_web_prom::PrometheusMetricsBuilder;
use dotenvy::dotenv;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use utoipa::OpenApi;
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;
use user_service::{db, cache, controllers, kafka};
use user_service::models::{User, NewUser, UpdateUser};

#[derive(OpenApi)]
#[openapi(
    info(title = "User Service API", version = "1.0.0", description = "User CRUD microservice"),
    paths(
        controllers::v1::user_controller::list_users,
        controllers::v1::user_controller::get_user,
        controllers::v1::user_controller::create_user,
        controllers::v1::user_controller::update_user,
        controllers::v1::user_controller::delete_user,
        controllers::health_controller::health_check,
    ),
    components(schemas(User, NewUser, UpdateUser)),
    tags(
        (name = "Users", description = "User CRUD operations"),
        (name = "Health", description = "Health check")
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;
impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let ip = "0.0.0.0";
    let server_port = 8082;
    let server_address = format!("http://{ip}:{server_port}");

    println!("Starting server at {server_address}");

    let db_pools = db::init_pools().await;
    let redis_pool = cache::init_redis().await;
    println!("Redis connected");

    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set");

    let kafka_broker = std::env::var("KAFKA_BROKER")
        .unwrap_or_else(|_| "localhost:9092".to_string());
    println!("Connecting to Kafka broker at {kafka_broker}");
    let consumer = kafka::create_consumer(&kafka_broker, "user-service-group");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let consumer_pools = db_pools.clone();
    let consumer_redis = redis_pool.clone();
    tokio::spawn(async move {
        kafka::start_consumer(consumer, consumer_pools, consumer_redis, shutdown_rx).await;
    });

    tokio::spawn(async move {
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to register SIGTERM handler");
        sigterm.recv().await;
        println!("SIGTERM received, signaling shutdown...");
        let _ = shutdown_tx.send(true);
    });

    let prometheus = PrometheusMetricsBuilder::new("user_service")
        .endpoint("/metrics")
        .build()
        .expect("Failed to create prometheus middleware");

    HttpServer::new(move || {
        App::new()
            .wrap(prometheus.clone())
            .app_data(web::Data::new(db_pools.clone()))
            .app_data(web::Data::new(redis_pool.clone()))
            .app_data(web::Data::new(jwt_secret.clone()))
            .configure(controllers::health_controller::init_routes)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
            .service(
                web::scope("/api/v1")
                    .configure(controllers::v1::user_controller::init_routes),
            )
    })
    .bind((ip, server_port))?
    .run()
    .await
}
