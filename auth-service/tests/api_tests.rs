use actix_web::{App, test, web};
use actix_web_prom::PrometheusMetricsBuilder;
use auth_service::{cache, config::AppConfig, controllers, db};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rdkafka::config::RDKafkaLogLevel;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const TEST_JWT_SECRET: &str = "test-secret-for-integration-tests";

fn create_test_producer() -> FutureProducer {
    let broker = std::env::var("KAFKA_BROKER").unwrap_or_else(|_| "127.0.0.1:9092".to_string());
    let mut config = ClientConfig::new();
    config
        .set("bootstrap.servers", &broker)
        .set("message.timeout.ms", "100")
        .set("socket.timeout.ms", "100")
        .set("queue.buffering.max.ms", "0");

    config
        .set_log_level(RDKafkaLogLevel::Emerg)
        .create()
        .expect("Failed to create Kafka producer for tests")
}

fn unique_email(prefix: &str) -> String {
    format!("{}-{}@example.com", prefix, Uuid::new_v4())
}

fn pkce_pair() -> (String, String) {
    let verifier = format!("verifier-{}", Uuid::new_v4());
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}

macro_rules! setup_app {
    () => {{
        dotenvy::dotenv().ok();
        let pool = db::init_pool().await;
        let pools = db::DbPools {
            write: pool.clone(),
            read: pool,
        };
        let redis = cache::init_redis().await;
        let config = AppConfig {
            jwt_secret: TEST_JWT_SECRET.to_string(),
            access_token_ttl_secs: 300,
            refresh_token_ttl_days: 7,
        };
        let producer = create_test_producer();
        let prometheus = PrometheusMetricsBuilder::new("auth_service_test")
            .endpoint("/metrics")
            .build()
            .unwrap();

        test::init_service(
            App::new()
                .wrap(prometheus.clone())
                .app_data(web::Data::new(pools))
                .app_data(web::Data::new(redis))
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(producer))
                .service(
                    web::scope("/api/v1")
                        .configure(controllers::v1::auth_controller::init_routes)
                        .configure(controllers::v1::client_controller::init_routes)
                        .configure(controllers::v1::oauth_controller::init_routes),
                ),
        )
        .await
    }};
}

#[actix_web::test]
async fn test_register_user_happy_path() {
    let app = setup_app!();
    let email = unique_email("auth-register");

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "email": email,
            "password": "password123"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["email"], email);
    assert!(body["id"].is_number());
}

#[actix_web::test]
async fn test_register_user_short_password_returns_400() {
    let app = setup_app!();
    let email = unique_email("short-pass");

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "email": email,
            "password": "short"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_register_user_duplicate_returns_409() {
    let app = setup_app!();
    let email = unique_email("duplicate-user");

    let first = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "email": email,
            "password": "password123"
        }))
        .to_request();
    let first_resp = test::call_service(&app, first).await;
    assert_eq!(first_resp.status(), 201);

    let second = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "email": email,
            "password": "password123"
        }))
        .to_request();
    let second_resp = test::call_service(&app, second).await;
    assert_eq!(second_resp.status(), 409);
}

#[actix_web::test]
async fn test_register_client_happy_path() {
    let app = setup_app!();

    let req = test::TestRequest::post()
        .uri("/api/v1/clients/register")
        .set_json(serde_json::json!({
            "client_name": "integration-client",
            "redirect_uri": "https://example.com/callback",
            "scopes": "read write"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["client_name"], "integration-client");
    assert_eq!(body["redirect_uri"], "https://example.com/callback");
    assert_eq!(body["scopes"], "read write");
    assert!(body["client_id"].as_str().unwrap_or("").len() > 10);
    assert!(body["client_secret"].as_str().unwrap_or("").len() > 10);
}

#[actix_web::test]
async fn test_oauth_authorization_code_flow() {
    let app = setup_app!();

    let email = unique_email("oauth-user");
    let password = "password123";

    let register_user = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "email": email,
            "password": password
        }))
        .to_request();
    let register_user_resp = test::call_service(&app, register_user).await;
    assert_eq!(register_user_resp.status(), 201);

    let register_client = test::TestRequest::post()
        .uri("/api/v1/clients/register")
        .set_json(serde_json::json!({
            "client_name": "oauth-integration-client",
            "redirect_uri": "https://example.com/callback",
            "scopes": "read write"
        }))
        .to_request();
    let register_client_resp = test::call_service(&app, register_client).await;
    assert_eq!(register_client_resp.status(), 201);

    let client_body: serde_json::Value = test::read_body_json(register_client_resp).await;
    let client_id = client_body["client_id"].as_str().unwrap().to_string();
    let client_secret = client_body["client_secret"].as_str().unwrap().to_string();
    let redirect_uri = client_body["redirect_uri"].as_str().unwrap().to_string();

    let (code_verifier, code_challenge) = pkce_pair();

    let authorize_uri = format!(
        "/api/v1/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope=read&state=state-123&code_challenge={}&code_challenge_method=S256",
        client_id,
        redirect_uri,
        code_challenge
    );

    let authorize_req = test::TestRequest::post()
        .uri(&authorize_uri)
        .set_json(serde_json::json!({
            "email": email,
            "password": password
        }))
        .to_request();
    let authorize_resp = test::call_service(&app, authorize_req).await;
    assert_eq!(authorize_resp.status(), 200);

    let authorize_body: serde_json::Value = test::read_body_json(authorize_resp).await;
    let code = authorize_body["code"].as_str().unwrap().to_string();
    assert_eq!(authorize_body["state"], "state-123");

    let token_req = test::TestRequest::post()
        .uri("/api/v1/oauth/token")
        .set_form(serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
            "client_id": client_id,
            "client_secret": client_secret,
            "code_verifier": code_verifier
        }))
        .to_request();
    let token_resp = test::call_service(&app, token_req).await;
    assert_eq!(token_resp.status(), 200);

    let token_body: serde_json::Value = test::read_body_json(token_resp).await;
    assert!(token_body["access_token"].as_str().unwrap_or("").len() > 10);
    assert!(token_body["refresh_token"].as_str().unwrap_or("").len() > 10);
    assert_eq!(token_body["token_type"], "Bearer");
    assert_eq!(token_body["scope"], "read");
}

#[actix_web::test]
async fn test_oauth_token_wrong_client_secret_returns_401() {
    let app = setup_app!();

    let register_client = test::TestRequest::post()
        .uri("/api/v1/clients/register")
        .set_json(serde_json::json!({
            "client_name": "bad-secret-client",
            "redirect_uri": "https://example.com/callback",
            "scopes": "read"
        }))
        .to_request();
    let register_client_resp = test::call_service(&app, register_client).await;
    assert_eq!(register_client_resp.status(), 201);

    let client_body: serde_json::Value = test::read_body_json(register_client_resp).await;
    let client_id = client_body["client_id"].as_str().unwrap().to_string();

    let token_req = test::TestRequest::post()
        .uri("/api/v1/oauth/token")
        .set_form(serde_json::json!({
            "grant_type": "client_credentials",
            "client_id": client_id,
            "client_secret": "wrong-secret",
            "scope": "read"
        }))
        .to_request();
    let token_resp = test::call_service(&app, token_req).await;
    assert_eq!(token_resp.status(), 401);

    let body: serde_json::Value = test::read_body_json(token_resp).await;
    assert_eq!(body["error"], "invalid_client");
}

#[actix_web::test]
async fn test_oauth_token_reuse_authorization_code_returns_400() {
    let app = setup_app!();

    let email = unique_email("reuse-code-user");
    let password = "password123";

    let register_user = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "email": email,
            "password": password
        }))
        .to_request();
    let register_user_resp = test::call_service(&app, register_user).await;
    assert_eq!(register_user_resp.status(), 201);

    let register_client = test::TestRequest::post()
        .uri("/api/v1/clients/register")
        .set_json(serde_json::json!({
            "client_name": "reuse-code-client",
            "redirect_uri": "https://example.com/callback",
            "scopes": "read"
        }))
        .to_request();
    let register_client_resp = test::call_service(&app, register_client).await;
    assert_eq!(register_client_resp.status(), 201);

    let client_body: serde_json::Value = test::read_body_json(register_client_resp).await;
    let client_id = client_body["client_id"].as_str().unwrap().to_string();
    let client_secret = client_body["client_secret"].as_str().unwrap().to_string();
    let redirect_uri = client_body["redirect_uri"].as_str().unwrap().to_string();

    let (code_verifier, code_challenge) = pkce_pair();

    let authorize_uri = format!(
        "/api/v1/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope=read&state=state-xyz&code_challenge={}&code_challenge_method=S256",
        client_id,
        redirect_uri,
        code_challenge
    );

    let authorize_req = test::TestRequest::post()
        .uri(&authorize_uri)
        .set_json(serde_json::json!({
            "email": email,
            "password": password
        }))
        .to_request();
    let authorize_resp = test::call_service(&app, authorize_req).await;
    assert_eq!(authorize_resp.status(), 200);

    let authorize_body: serde_json::Value = test::read_body_json(authorize_resp).await;
    let code = authorize_body["code"].as_str().unwrap().to_string();

    let first_exchange = test::TestRequest::post()
        .uri("/api/v1/oauth/token")
        .set_form(serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
            "client_id": client_id,
            "client_secret": client_secret,
            "code_verifier": code_verifier
        }))
        .to_request();
    let first_exchange_resp = test::call_service(&app, first_exchange).await;
    assert_eq!(first_exchange_resp.status(), 200);

    let second_exchange = test::TestRequest::post()
        .uri("/api/v1/oauth/token")
        .set_form(serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
            "client_id": client_id,
            "client_secret": client_secret,
            "code_verifier": code_verifier
        }))
        .to_request();
    let second_exchange_resp = test::call_service(&app, second_exchange).await;
    assert_eq!(second_exchange_resp.status(), 400);

    let body: serde_json::Value = test::read_body_json(second_exchange_resp).await;
    assert_eq!(body["error"], "invalid_grant");
}

#[actix_web::test]
async fn test_metrics_endpoint() {
    let app = setup_app!();

    let req = test::TestRequest::get()
        .uri("/metrics")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
