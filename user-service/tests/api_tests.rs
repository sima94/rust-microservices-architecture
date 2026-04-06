use actix_web::{App, test, web};
use actix_web_prom::PrometheusMetricsBuilder;
use user_service::{cache, controllers, db};

const TEST_JWT_SECRET: &str = "test-secret-for-integration-tests";

fn create_test_token(scopes: &str) -> String {
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Claims {
        sub: String,
        email: Option<String>,
        client_id: String,
        scopes: String,
        exp: usize,
        iat: usize,
    }

    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: "test-user-1".into(),
        email: Some("test@example.com".into()),
        client_id: "test-client".into(),
        scopes: scopes.into(),
        exp: now + 3600,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap()
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
        let jwt_secret = TEST_JWT_SECRET.to_string();
        let prometheus = PrometheusMetricsBuilder::new("user_service_test")
            .endpoint("/metrics")
            .build()
            .unwrap();
        test::init_service(
            App::new()
                .wrap(prometheus.clone())
                .app_data(web::Data::new(pools))
                .app_data(web::Data::new(redis))
                .app_data(web::Data::new(jwt_secret))
                .service(
                    web::scope("/api/v1").configure(controllers::v1::user_controller::init_routes),
                ),
        )
        .await
    }};
}

#[actix_web::test]
async fn test_create_user() {
    let app = setup_app!();
    let token = create_test_token("read write");

    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({"name": "Integration Test", "email": "integration@test.com"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "Integration Test");
    assert_eq!(body["email"], "integration@test.com");
    assert!(body["id"].is_number());
}

#[actix_web::test]
async fn test_get_user() {
    let app = setup_app!();
    let token = create_test_token("read write");

    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({"name": "GetTest", "email": "get@test.com"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let created: serde_json::Value = test::read_body_json(resp).await;
    let user_id = created["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "GetTest");
}

#[actix_web::test]
async fn test_list_users() {
    let app = setup_app!();
    let token = create_test_token("read");

    let req = test::TestRequest::get()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn test_update_user() {
    let app = setup_app!();
    let token = create_test_token("read write");

    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({"name": "BeforeUpdate", "email": "before@test.com"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let created: serde_json::Value = test::read_body_json(resp).await;
    let user_id = created["id"].as_i64().unwrap();

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({"name": "AfterUpdate"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "AfterUpdate");
    assert_eq!(body["email"], "before@test.com");
}

#[actix_web::test]
async fn test_delete_user() {
    let app = setup_app!();
    let token = create_test_token("read write");

    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({"name": "ToDelete", "email": "delete@test.com"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let created: serde_json::Value = test::read_body_json(resp).await;
    let user_id = created["id"].as_i64().unwrap();

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_get_nonexistent_user() {
    let app = setup_app!();
    let token = create_test_token("read");

    let req = test::TestRequest::get()
        .uri("/api/v1/users/999999")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_unauthorized_without_token() {
    let app = setup_app!();

    let req = test::TestRequest::get().uri("/api/v1/users").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_metrics_endpoint() {
    let app = setup_app!();

    let req = test::TestRequest::get().uri("/metrics").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
