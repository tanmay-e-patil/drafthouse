use actix_web::{App, http::StatusCode, test, web};
use auth_networking::routes;
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{RegisterRequest, ResendVerificationRequest, VerifyEmailRequest};
use serial_test::serial;
use sqlx::PgPool;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

struct TestEnv {
    pool: PgPool,
    _container: ContainerAsync<Postgres>,
}

impl TestEnv {
    async fn new() -> Self {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);
        let pool = PgPool::connect(&url).await.unwrap();
        sqlx::migrate!("../../../migrations/postgres")
            .run(&pool)
            .await
            .unwrap();
        Self {
            pool,
            _container: container,
        }
    }
}

macro_rules! make_app {
    ($env:expr) => {{
        let pool = $env.pool.clone();
        test::init_service(App::new().configure(move |cfg| {
            let dal = web::Data::new(SqlxPostGresDescriptor { pool: pool.clone() });
            routes::configure(cfg, dal);
        }))
        .await
    }};
}

// ── register ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn register_success() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/register")
        .set_json(RegisterRequest {
            email: "new@example.com".into(),
            password: "password123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["email"], "new@example.com");
    assert!(body["user_id"].is_string());
}

#[tokio::test]
async fn register_duplicate_email_returns_409() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let payload = RegisterRequest {
        email: "dup@example.com".into(),
        password: "password123".into(),
    };

    let first = test::TestRequest::post()
        .uri("/auth/register")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, first).await;

    let second = test::TestRequest::post()
        .uri("/auth/register")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, second).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn register_empty_email_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/register")
        .set_json(RegisterRequest {
            email: "".into(),
            password: "password123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_short_password_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/register")
        .set_json(RegisterRequest {
            email: "short@example.com".into(),
            password: "abc".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── verify-email ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn verify_email_success() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let user = sqlx::query_as::<_, kernel::User>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2)
         RETURNING id, email, password_hash, email_verified_at, created_at",
    )
    .bind("toverify@example.com")
    .bind("irrelevant_hash")
    .fetch_one(&env.pool)
    .await
    .unwrap();

    let raw_token = "integration_test_raw_token_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let token_hash = auth_core::token::hash_token(raw_token);

    sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
         VALUES ($1, $2, NOW() + INTERVAL '24 hours')",
    )
    .bind(user.id)
    .bind(&token_hash)
    .execute(&env.pool)
    .await
    .unwrap();

    let req = test::TestRequest::post()
        .uri("/auth/verify-email")
        .set_json(VerifyEmailRequest {
            token: raw_token.into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let verified_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT email_verified_at FROM users WHERE id = $1")
            .bind(user.id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert!(
        verified_at.is_some(),
        "user should be marked verified in DB"
    );
}

#[tokio::test]
async fn verify_email_invalid_token_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/verify-email")
        .set_json(VerifyEmailRequest {
            token: "nonexistent_token".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn verify_email_expired_token_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let user = sqlx::query_as::<_, kernel::User>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2)
         RETURNING id, email, password_hash, email_verified_at, created_at",
    )
    .bind("expired@example.com")
    .bind("irrelevant_hash")
    .fetch_one(&env.pool)
    .await
    .unwrap();

    let raw_token = "integration_test_expired_token_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let token_hash = auth_core::token::hash_token(raw_token);

    sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
         VALUES ($1, $2, NOW() - INTERVAL '1 hour')",
    )
    .bind(user.id)
    .bind(&token_hash)
    .execute(&env.pool)
    .await
    .unwrap();

    let req = test::TestRequest::post()
        .uri("/auth/verify-email")
        .set_json(VerifyEmailRequest {
            token: raw_token.into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── resend-verification ───────────────────────────────────────────────────────

#[tokio::test]
async fn resend_unknown_email_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/resend-verification")
        .set_json(ResendVerificationRequest {
            email: "ghost@example.com".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn resend_already_verified_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    sqlx::query(
        "INSERT INTO users (email, password_hash, email_verified_at)
         VALUES ($1, $2, NOW())",
    )
    .bind("alreadyverified@example.com")
    .bind("irrelevant_hash")
    .execute(&env.pool)
    .await
    .unwrap();

    let req = test::TestRequest::post()
        .uri("/auth/resend-verification")
        .set_json(ResendVerificationRequest {
            email: "alreadyverified@example.com".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn resend_success() {
    let env = TestEnv::new().await;

    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/emails"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "test-id"})),
        )
        .mount(&mock_server)
        .await;

    unsafe {
        std::env::set_var("RESEND_API_BASE_URL", mock_server.uri());
    }

    let app = make_app!(env);

    sqlx::query("INSERT INTO users (email, password_hash) VALUES ($1, $2)")
        .bind("resend@example.com")
        .bind("irrelevant_hash")
        .execute(&env.pool)
        .await
        .unwrap();

    let req = test::TestRequest::post()
        .uri("/auth/resend-verification")
        .set_json(ResendVerificationRequest {
            email: "resend@example.com".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    unsafe {
        std::env::remove_var("RESEND_API_BASE_URL");
    }
}
