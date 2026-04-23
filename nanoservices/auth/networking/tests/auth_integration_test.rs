use actix_web::{App, http::StatusCode, test, web};
use auth_networking::routes;
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{
    ChangePasswordRequest, DeleteAccountRequest, ForgotPasswordRequest, LoginRequest,
    RegisterRequest, ResendVerificationRequest, ResetPasswordRequest, VerifyEmailRequest,
};
use serial_test::serial;
use sqlx::PgPool;
use sqlx::types::Uuid;
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
         RETURNING id, email, password_hash, email_verified_at, created_at, welcome_doc_created",
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
         RETURNING id, email, password_hash, email_verified_at, created_at, welcome_doc_created",
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

// ── helpers ───────────────────────────────────────────────────────────────────

async fn create_verified_user(pool: &sqlx::PgPool, email: &str, password: &str) {
    let hash = auth_core::password::hash_password(password).unwrap();
    sqlx::query(
        "INSERT INTO users (email, password_hash, email_verified_at, welcome_doc_created) VALUES ($1, $2, NOW(), true)",
    )
    .bind(email)
    .bind(&hash)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_verified_user_record(
    pool: &sqlx::PgPool,
    email: &str,
    password: &str,
) -> kernel::User {
    let hash = auth_core::password::hash_password(password).unwrap();
    sqlx::query_as::<_, kernel::User>(
        "INSERT INTO users (email, password_hash, email_verified_at, welcome_doc_created)
         VALUES ($1, $2, NOW(), true)
         RETURNING id, email, password_hash, email_verified_at, created_at, welcome_doc_created",
    )
    .bind(email)
    .bind(&hash)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn extract_refresh_cookie(resp: &actix_web::dev::ServiceResponse) -> Option<String> {
    resp.response()
        .cookies()
        .find(|c| c.name() == "refresh_token")
        .map(|c| c.value().to_string())
}

// ── login ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn login_valid_credentials_returns_200_and_tokens() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "login@example.com", "password123").await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_json(LoginRequest {
            email: "login@example.com".into(),
            password: "password123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let cookie = extract_refresh_cookie(&resp);
    assert!(cookie.is_some(), "refresh_token cookie must be set");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["access_token"].is_string());
    assert_eq!(body["token_type"], "Bearer");
}

#[tokio::test]
async fn login_wrong_password_returns_401() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "wrongpw@example.com", "correct123").await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_json(LoginRequest {
            email: "wrongpw@example.com".into(),
            password: "wrongpassword".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_unknown_email_returns_401() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_json(LoginRequest {
            email: "nobody@example.com".into(),
            password: "password123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_unverified_account_returns_403() {
    let env = TestEnv::new().await;
    let hash = auth_core::password::hash_password("password123").unwrap();
    sqlx::query("INSERT INTO users (email, password_hash) VALUES ($1, $2)")
        .bind("unverified2@example.com")
        .bind(&hash)
        .execute(&env.pool)
        .await
        .unwrap();

    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_json(LoginRequest {
            email: "unverified2@example.com".into(),
            password: "password123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── refresh ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_valid_cookie_returns_new_access_token() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "refreshuser@example.com", "password123").await;
    let app = make_app!(env);

    let login_req = test::TestRequest::post()
        .uri("/auth/login")
        .set_json(LoginRequest {
            email: "refreshuser@example.com".into(),
            password: "password123".into(),
        })
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let refresh_cookie = extract_refresh_cookie(&login_resp).unwrap();

    let refresh_req = test::TestRequest::post()
        .uri("/auth/refresh")
        .cookie(actix_web::cookie::Cookie::new(
            "refresh_token",
            refresh_cookie,
        ))
        .to_request();

    let refresh_resp = test::call_service(&app, refresh_req).await;
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(refresh_resp).await;
    assert!(body["access_token"].is_string());
}

#[tokio::test]
async fn refresh_missing_cookie_returns_401() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post().uri("/auth/refresh").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_invalid_token_returns_401() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/refresh")
        .cookie(actix_web::cookie::Cookie::new(
            "refresh_token",
            "bogus_token",
        ))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── login → expire → refresh → retry cycle (integration) ─────────────────────

#[tokio::test]
async fn login_refresh_cycle_token_rotates() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "cycle@example.com", "password123").await;
    let app = make_app!(env);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "cycle@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let first_cookie = extract_refresh_cookie(&login_resp).unwrap();

    let refresh_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .cookie(actix_web::cookie::Cookie::new(
                "refresh_token",
                first_cookie.clone(),
            ))
            .to_request(),
    )
    .await;
    assert_eq!(refresh_resp.status(), StatusCode::OK);
    let second_cookie = extract_refresh_cookie(&refresh_resp).unwrap();
    assert_ne!(first_cookie, second_cookie, "token must rotate");

    // Old token must be invalidated
    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .cookie(actix_web::cookie::Cookie::new(
                "refresh_token",
                first_cookie,
            ))
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

// ── logout ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn logout_revokes_single_refresh_token() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "logoutuser@example.com", "password123").await;
    let app = make_app!(env);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "logoutuser@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let cookie = extract_refresh_cookie(&login_resp).unwrap();

    let logout_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/logout")
            .cookie(actix_web::cookie::Cookie::new("refresh_token", cookie))
            .to_request(),
    )
    .await;
    assert_eq!(logout_resp.status(), StatusCode::OK);

    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .cookie(actix_web::cookie::Cookie::new(
                "refresh_token",
                extract_refresh_cookie(&login_resp).unwrap(),
            ))
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

// ── logout-all ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn logout_all_revokes_all_sessions() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "logoutall@example.com", "password123").await;
    let app = make_app!(env);

    let login_resp_1 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "logoutall@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let body_1: serde_json::Value = test::read_body_json(login_resp_1).await;
    let access_token = body_1["access_token"].as_str().unwrap();

    let login_resp_2 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "logoutall@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let cookie = extract_refresh_cookie(&login_resp_2).unwrap();

    let logout_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/logout-all")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .to_request(),
    )
    .await;
    assert_eq!(logout_resp.status(), StatusCode::OK);

    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .cookie(actix_web::cookie::Cookie::new("refresh_token", cookie))
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

// ── forgot-password ─────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn forgot_password_known_email_returns_200() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "forgot@example.com", "password123").await;

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

    let req = test::TestRequest::post()
        .uri("/auth/forgot-password")
        .set_json(ForgotPasswordRequest {
            email: "forgot@example.com".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["message"].as_str().unwrap().contains("reset link"));

    unsafe {
        std::env::remove_var("RESEND_API_BASE_URL");
    }
}

#[tokio::test]
async fn forgot_password_unknown_email_returns_200_generic() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/forgot-password")
        .set_json(ForgotPasswordRequest {
            email: "nobody@example.com".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["message"].as_str().unwrap().contains("reset link"));
}

// ── reset-password ──────────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn reset_password_full_flow() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "resetuser@example.com", "oldPassword123").await;

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

    let forgot_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/forgot-password")
            .set_json(ForgotPasswordRequest {
                email: "resetuser@example.com".into(),
            })
            .to_request(),
    )
    .await;
    assert_eq!(forgot_resp.status(), StatusCode::OK);

    unsafe {
        std::env::remove_var("RESEND_API_BASE_URL");
    }

    let raw_token = "reset_test_token_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let token_hash = auth_core::token::hash_token(raw_token);

    sqlx::query("INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) SELECT id, $1, NOW() + INTERVAL '15 minutes' FROM users WHERE email = 'resetuser@example.com'")
        .bind(&token_hash)
        .execute(&env.pool)
        .await
        .unwrap();

    let reset_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/reset-password")
            .set_json(ResetPasswordRequest {
                token: raw_token.into(),
                new_password: "brandNewPassword456".into(),
            })
            .to_request(),
    )
    .await;
    assert_eq!(reset_resp.status(), StatusCode::OK);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "resetuser@example.com".into(),
                password: "brandNewPassword456".into(),
            })
            .to_request(),
    )
    .await;
    assert_eq!(login_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn reset_password_expired_token_returns_400() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "expiredreset@example.com", "password123").await;
    let app = make_app!(env);

    let raw_token = "expired_reset_token_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let token_hash = auth_core::token::hash_token(raw_token);

    sqlx::query("INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) SELECT id, $1, NOW() - INTERVAL '1 hour' FROM users WHERE email = 'expiredreset@example.com'")
        .bind(&token_hash)
        .execute(&env.pool)
        .await
        .unwrap();

    let req = test::TestRequest::post()
        .uri("/auth/reset-password")
        .set_json(ResetPasswordRequest {
            token: raw_token.into(),
            new_password: "newPassword123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reset_password_used_token_returns_400() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "usedreset@example.com", "password123").await;
    let app = make_app!(env);

    let raw_token = "used_reset_token_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let token_hash = auth_core::token::hash_token(raw_token);

    sqlx::query("INSERT INTO password_reset_tokens (user_id, token_hash, expires_at, used_at) SELECT id, $1, NOW() + INTERVAL '15 minutes', NOW() FROM users WHERE email = 'usedreset@example.com'")
        .bind(&token_hash)
        .execute(&env.pool)
        .await
        .unwrap();

    let req = test::TestRequest::post()
        .uri("/auth/reset-password")
        .set_json(ResetPasswordRequest {
            token: raw_token.into(),
            new_password: "newPassword123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reset_password_invalid_token_returns_400() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/auth/reset-password")
        .set_json(ResetPasswordRequest {
            token: "nonexistent_token".into(),
            new_password: "newPassword123".into(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reset_password_revokes_all_sessions() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "revokesessions@example.com", "password123").await;
    let app = make_app!(env);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "revokesessions@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let cookie = extract_refresh_cookie(&login_resp).unwrap();

    let raw_token = "revokesessions_token_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let token_hash = auth_core::token::hash_token(raw_token);

    sqlx::query("INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) SELECT id, $1, NOW() + INTERVAL '15 minutes' FROM users WHERE email = 'revokesessions@example.com'")
        .bind(&token_hash)
        .execute(&env.pool)
        .await
        .unwrap();

    test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/reset-password")
            .set_json(ResetPasswordRequest {
                token: raw_token.into(),
                new_password: "afterResetPassword".into(),
            })
            .to_request(),
    )
    .await;

    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .cookie(actix_web::cookie::Cookie::new("refresh_token", cookie))
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

// ── me/account management ───────────────────────────────────────────────────

#[tokio::test]
async fn get_me_returns_current_profile() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "me@example.com", "password123").await;
    let app = make_app!(env);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "me@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(login_resp).await;
    let access_token = body["access_token"].as_str().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/auth/me")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["email"], "me@example.com");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn change_password_updates_hash_and_revokes_refresh_tokens() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "changepw@example.com", "oldpassword123").await;
    let app = make_app!(env);

    let first_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "changepw@example.com".into(),
                password: "oldpassword123".into(),
            })
            .to_request(),
    )
    .await;
    let first_cookie = extract_refresh_cookie(&first_login).unwrap();
    let first_body: serde_json::Value = test::read_body_json(first_login).await;
    let access_token = first_body["access_token"].as_str().unwrap();

    let before_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE email = $1")
            .bind("changepw@example.com")
            .fetch_one(&env.pool)
            .await
            .unwrap();

    let change_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/me/password")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .set_json(ChangePasswordRequest {
                current_password: "oldpassword123".into(),
                new_password: "newpassword456".into(),
            })
            .to_request(),
    )
    .await;
    assert_eq!(change_resp.status(), StatusCode::OK);
    assert_eq!(
        change_resp
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "refresh_token")
            .map(|cookie| cookie.max_age().map(|age| age.whole_seconds())),
        Some(Some(0))
    );

    let after_hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE email = $1")
        .bind("changepw@example.com")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_ne!(before_hash, after_hash);

    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .cookie(actix_web::cookie::Cookie::new(
                "refresh_token",
                first_cookie,
            ))
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn change_password_wrong_current_password_returns_400() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "changepwwrong@example.com", "oldpassword123").await;
    let app = make_app!(env);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "changepwwrong@example.com".into(),
                password: "oldpassword123".into(),
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(login_resp).await;
    let access_token = body["access_token"].as_str().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/me/password")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .set_json(ChangePasswordRequest {
                current_password: "wrong-password".into(),
                new_password: "newpassword456".into(),
            })
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_account_removes_user_and_related_rows() {
    let env = TestEnv::new().await;
    let owner = create_verified_user_record(&env.pool, "delete@example.com", "password123").await;
    let member = create_verified_user_record(&env.pool, "member@example.com", "password123").await;
    let app = make_app!(env);

    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO documents (owner_id, title, content) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(owner.id)
    .bind("Owned doc")
    .bind("# owned")
    .fetch_one(&env.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO document_members (doc_id, user_id, role) VALUES ($1, $2, 'editor')")
        .bind(document_id)
        .bind(member.id)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, NOW() + INTERVAL '1 day')",
    )
    .bind(owner.id)
    .bind("manual-refresh-hash")
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, NOW() + INTERVAL '15 minutes')",
    )
    .bind(owner.id)
    .bind("manual-reset-hash")
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, NOW() + INTERVAL '1 day')",
    )
    .bind(owner.id)
    .bind("manual-verify-hash")
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ws_tickets (token_hash, doc_id, user_id, expires_at) VALUES ($1, $2, $3, NOW() + INTERVAL '1 hour')",
    )
    .bind("manual-ws-hash")
    .bind(document_id)
    .bind(owner.id)
    .execute(&env.pool)
    .await
    .unwrap();

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "delete@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(login_resp).await;
    let access_token = body["access_token"].as_str().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/auth/me")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .set_json(DeleteAccountRequest {
                current_password: "password123".into(),
            })
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = $1")
        .bind(owner.id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    let document_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM documents WHERE owner_id = $1")
            .bind(owner.id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let member_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM document_members WHERE doc_id = $1")
            .bind(document_id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let refresh_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(owner.id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let reset_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1")
            .bind(owner.id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let verify_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM email_verification_tokens WHERE user_id = $1")
            .bind(owner.id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let ws_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ws_tickets WHERE user_id = $1")
        .bind(owner.id)
        .fetch_one(&env.pool)
        .await
        .unwrap();

    assert_eq!(user_count, 0);
    assert_eq!(document_count, 0);
    assert_eq!(member_count, 0);
    assert_eq!(refresh_count, 0);
    assert_eq!(reset_count, 0);
    assert_eq!(verify_count, 0);
    assert_eq!(ws_count, 0);
}

#[tokio::test]
async fn delete_account_wrong_password_returns_400() {
    let env = TestEnv::new().await;
    create_verified_user(&env.pool, "deletewrong@example.com", "password123").await;
    let app = make_app!(env);

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "deletewrong@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(login_resp).await;
    let access_token = body["access_token"].as_str().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/auth/me")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .set_json(DeleteAccountRequest {
                current_password: "wrong-password".into(),
            })
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn export_account_data_returns_202_and_sends_email() {
    let env = TestEnv::new().await;
    let owner = create_verified_user_record(&env.pool, "export@example.com", "password123").await;
    sqlx::query("INSERT INTO documents (owner_id, title, content) VALUES ($1, $2, $3)")
        .bind(owner.id)
        .bind("Export Doc")
        .bind("# exported")
        .execute(&env.pool)
        .await
        .unwrap();

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

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginRequest {
                email: "export@example.com".into(),
                password: "password123".into(),
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(login_resp).await;
    let access_token = body["access_token"].as_str().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/me/export")
            .insert_header(("Authorization", format!("Bearer {}", access_token)))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    for _ in 0..100 {
        if mock_server.received_requests().await.unwrap().len() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let requests = mock_server.received_requests().await.unwrap();
    assert!(!requests.is_empty(), "export email request should be sent");

    unsafe {
        std::env::remove_var("RESEND_API_BASE_URL");
    }
}
