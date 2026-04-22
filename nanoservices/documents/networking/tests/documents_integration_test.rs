use actix_web::{App, http::StatusCode, test, web};
use auth_networking::routes as auth_routes;
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{
    CreateDocumentRequest, CreateInviteLinkRequest, LoginRequest, MemberRole,
    UpdateDocumentContentRequest, UpdateDocumentRequest, UpdateMemberRoleRequest,
};
use sqlx::PgPool;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};
use uuid::Uuid;

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
            auth_routes::configure(cfg, dal.clone());
            documents_networking::routes::configure(cfg, dal);
        }))
        .await
    }};
}

macro_rules! login_token {
    ($app:expr, $email:expr, $password:expr) => {{
        let resp = test::call_service(
            $app,
            test::TestRequest::post()
                .uri("/auth/login")
                .set_json(LoginRequest {
                    email: $email.into(),
                    password: $password.into(),
                })
                .to_request(),
        )
        .await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["access_token"].as_str().unwrap().to_string()
    }};
}

macro_rules! create_verified_user {
    ($pool:expr, $email:expr, $password:expr) => {{
        let hash = auth_core::password::hash_password($password).unwrap();
        sqlx::query(
            "INSERT INTO users (email, password_hash, email_verified_at, welcome_doc_created) VALUES ($1, $2, NOW(), true)",
        )
        .bind($email)
        .bind(&hash)
        .execute($pool)
        .await
        .unwrap();
    }};
}

// ── create document ──────────────────────────────────────────────────────

#[tokio::test]
async fn create_document_returns_201() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "docuser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "docuser@example.com", "password123");

    let req = test::TestRequest::post()
        .uri("/documents")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(CreateDocumentRequest {
            title: Some("My Doc".into()),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["title"], "My Doc");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn create_document_default_title() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "defaultitle@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "defaultitle@example.com", "password123");

    let req = test::TestRequest::post()
        .uri("/documents")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(CreateDocumentRequest { title: None })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["title"], "Untitled");
}

#[tokio::test]
async fn create_document_unauthenticated_returns_401() {
    let env = TestEnv::new().await;
    let app = make_app!(env);

    let req = test::TestRequest::post()
        .uri("/documents")
        .set_json(CreateDocumentRequest {
            title: Some("No Auth".into()),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── list documents ───────────────────────────────────────────────────────

#[tokio::test]
async fn list_documents_returns_owned() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "listuser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "listuser@example.com", "password123");

    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("listuser@example.com")
        .fetch_one(&env.pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO documents (owner_id, title) VALUES ($1, 'Doc A')")
        .bind(user_id)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO documents (owner_id, title) VALUES ($1, 'Doc B')")
        .bind(user_id)
        .execute(&env.pool)
        .await
        .unwrap();

    let req = test::TestRequest::get()
        .uri("/documents")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert!(!body["has_more"].as_bool().unwrap());
}

#[tokio::test]
async fn list_documents_empty() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "emptylist@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "emptylist@example.com", "password123");

    let req = test::TestRequest::get()
        .uri("/documents")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
    assert!(!body["has_more"].as_bool().unwrap());
}

// ── get document ─────────────────────────────────────────────────────────

#[tokio::test]
async fn get_document_by_id() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "getuser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "getuser@example.com", "password123");

    let req = test::TestRequest::post()
        .uri("/documents")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(CreateDocumentRequest {
            title: Some("Get Me".into()),
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let doc_id = body["id"].as_str().unwrap();

    let get_req = test::TestRequest::get()
        .uri(&format!("/documents/{}", doc_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    assert_eq!(get_resp.status(), StatusCode::OK);

    let get_body: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(get_body["title"], "Get Me");
}

#[tokio::test]
async fn get_document_not_found() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "notfound@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "notfound@example.com", "password123");

    let req = test::TestRequest::get()
        .uri(&format!("/documents/{}", Uuid::new_v4()))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── update document ──────────────────────────────────────────────────────

#[tokio::test]
async fn update_document_title() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "updateuser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "updateuser@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateDocumentRequest {
                title: Some("Original".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap();

    let update_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(UpdateDocumentRequest {
                title: Some("Renamed".into()),
                is_public: None,
            })
            .to_request(),
    )
    .await;

    assert_eq!(update_resp.status(), StatusCode::OK);
    let update_body: serde_json::Value = test::read_body_json(update_resp).await;
    assert_eq!(update_body["title"], "Renamed");
}

#[tokio::test]
async fn update_document_non_owner_returns_403() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "owner@example.com", "password123");
    create_verified_user!(&env.pool, "other@example.com", "password123");
    let app = make_app!(env);

    let owner_token = login_token!(&app, "owner@example.com", "password123");
    let other_token = login_token!(&app, "other@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(CreateDocumentRequest {
                title: Some("Mine".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap();

    let update_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", other_token)))
            .set_json(UpdateDocumentRequest {
                title: Some("Hacked".into()),
                is_public: None,
            })
            .to_request(),
    )
    .await;

    assert_eq!(update_resp.status(), StatusCode::FORBIDDEN);
}

// ── delete document ──────────────────────────────────────────────────────

#[tokio::test]
async fn delete_document_owner_returns_204() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "deluser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "deluser@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateDocumentRequest {
                title: Some("To Delete".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap();

    let del_resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;

    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    let get_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_document_non_owner_returns_403() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "delowner@example.com", "password123");
    create_verified_user!(&env.pool, "delother@example.com", "password123");
    let app = make_app!(env);

    let owner_token = login_token!(&app, "delowner@example.com", "password123");
    let other_token = login_token!(&app, "delother@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(CreateDocumentRequest {
                title: Some("Protected".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap();

    let del_resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", other_token)))
            .to_request(),
    )
    .await;

    assert_eq!(del_resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_document_not_found_returns_404() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "del404@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "del404@example.com", "password123");

    let del_resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}", Uuid::new_v4()))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;

    assert_eq!(del_resp.status(), StatusCode::NOT_FOUND);
}

// ── pagination ───────────────────────────────────────────────────────────

#[tokio::test]
async fn pagination_with_20_plus_docs() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "paguser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "paguser@example.com", "password123");

    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("paguser@example.com")
        .fetch_one(&env.pool)
        .await
        .unwrap();

    for i in 0..25 {
        sqlx::query("INSERT INTO documents (owner_id, title, updated_at) VALUES ($1, $2, now() - ($3 || ' seconds')::interval)")
            .bind(user_id)
            .bind(format!("Doc {}", i))
            .bind(i * 10)
            .execute(&env.pool)
            .await
            .unwrap();
    }

    let first_req = test::TestRequest::get()
        .uri("/documents?limit=20")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let first_resp = test::call_service(&app, first_req).await;
    assert_eq!(first_resp.status(), StatusCode::OK);
    let first_body: serde_json::Value = test::read_body_json(first_resp).await;
    assert_eq!(first_body["data"].as_array().unwrap().len(), 20);
    assert!(first_body["has_more"].as_bool().unwrap());
    let next_cursor = first_body["next_cursor"].as_str().unwrap();

    let second_req = test::TestRequest::get()
        .uri(&format!("/documents?limit=20&cursor={}", next_cursor))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let second_resp = test::call_service(&app, second_req).await;
    assert_eq!(second_resp.status(), StatusCode::OK);
    let second_body: serde_json::Value = test::read_body_json(second_resp).await;
    assert_eq!(second_body["data"].as_array().unwrap().len(), 5);
    assert!(!second_body["has_more"].as_bool().unwrap());
}

// ── full CRUD cycle ─────────────────────────────────────────────────────

#[tokio::test]
async fn full_crud_cycle() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "crud@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "crud@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateDocumentRequest {
                title: Some("CRUD Doc".into()),
            })
            .to_request(),
    )
    .await;
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap().to_string();

    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body: serde_json::Value = test::read_body_json(list_resp).await;
    assert_eq!(list_body["data"].as_array().unwrap().len(), 1);

    let update_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(UpdateDocumentRequest {
                title: Some("Updated Doc".into()),
                is_public: Some(true),
            })
            .to_request(),
    )
    .await;
    assert_eq!(update_resp.status(), StatusCode::OK);
    let update_body: serde_json::Value = test::read_body_json(update_resp).await;
    assert_eq!(update_body["title"], "Updated Doc");
    assert!(update_body["is_public"].as_bool().unwrap());

    let del_resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    let list_after_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    let list_after_body: serde_json::Value = test::read_body_json(list_after_resp).await;
    assert_eq!(list_after_body["data"].as_array().unwrap().len(), 0);
}

// ── document content ────────────────────────────────────────────────────

#[tokio::test]
async fn get_document_content_returns_empty_for_new_doc() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "contentuser@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "contentuser@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateDocumentRequest {
                title: Some("Empty Content".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap();

    let get_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/content", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::OK);

    let get_body: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(get_body["content"], "");
}

#[tokio::test]
async fn update_and_get_document_content() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "contentupd@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "contentupd@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateDocumentRequest {
                title: Some("With Content".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap().to_string();

    let update_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/documents/{}/content", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(UpdateDocumentContentRequest {
                content: "# Hello\n\nWorld".into(),
            })
            .to_request(),
    )
    .await;
    assert_eq!(update_resp.status(), StatusCode::OK);

    let get_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/content", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::OK);

    let get_body: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(get_body["content"], "# Hello\n\nWorld");
}

#[tokio::test]
async fn get_document_content_not_found() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "content404@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "content404@example.com", "password123");

    let get_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/content", Uuid::new_v4()))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_document_content_not_found() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "contentupd404@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "contentupd404@example.com", "password123");

    let update_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/documents/{}/content", Uuid::new_v4()))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(UpdateDocumentContentRequest {
                content: "nope".into(),
            })
            .to_request(),
    )
    .await;
    assert_eq!(update_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_document_content_multiple_times() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "multicontent@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "multicontent@example.com", "password123");

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/documents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateDocumentRequest {
                title: Some("Multi".into()),
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let doc_id = create_body["id"].as_str().unwrap().to_string();

    for i in 0..5 {
        let update_resp = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/documents/{}/content", doc_id))
                .insert_header(("Authorization", format!("Bearer {}", token)))
                .set_json(UpdateDocumentContentRequest {
                    content: format!("Version {}", i),
                })
                .to_request(),
        )
        .await;
        assert_eq!(update_resp.status(), StatusCode::OK);
    }

    let get_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/content", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    let get_body: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(get_body["content"], "Version 4");
}

// ── invite links ──────────────────────────────────────────────────────────

macro_rules! create_doc {
    ($app:expr, $token:expr) => {{
        let resp = test::call_service(
            $app,
            test::TestRequest::post()
                .uri("/documents")
                .insert_header(("Authorization", format!("Bearer {}", $token)))
                .set_json(CreateDocumentRequest {
                    title: Some("Test Doc".into()),
                })
                .to_request(),
        )
        .await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["id"].as_str().unwrap().to_string()
    }};
}

#[tokio::test]
async fn create_invite_link_returns_201_with_token() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "invowner@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "invowner@example.com", "password123");
    let doc_id = create_doc!(&app, &token);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Editor,
                expires_at: None,
                max_uses: None,
            })
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["token"].as_str().is_some());
    assert_eq!(body["role"], "editor");
}

#[tokio::test]
async fn create_invite_link_non_owner_returns_403() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "invown2@example.com", "password123");
    create_verified_user!(&env.pool, "invother@example.com", "password123");
    let app = make_app!(env);
    let owner_token = login_token!(&app, "invown2@example.com", "password123");
    let other_token = login_token!(&app, "invother@example.com", "password123");
    let doc_id = create_doc!(&app, &owner_token);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", other_token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Viewer,
                expires_at: None,
                max_uses: None,
            })
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_invite_links_returns_active_links() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "listinv@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "listinv@example.com", "password123");
    let doc_id = create_doc!(&app, &token);

    test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Viewer,
                expires_at: None,
                max_uses: Some(5),
            })
            .to_request(),
    )
    .await;

    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(list_resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["max_uses"], 5);
}

#[tokio::test]
async fn revoke_invite_link_returns_204() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "revokeinv@example.com", "password123");
    let app = make_app!(env);
    let token = login_token!(&app, "revokeinv@example.com", "password123");
    let doc_id = create_doc!(&app, &token);

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Editor,
                expires_at: None,
                max_uses: None,
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let inv_token = create_body["token"].as_str().unwrap().to_string();

    let revoke_resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}/invites/{}", doc_id, inv_token))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(revoke_resp.status(), StatusCode::NO_CONTENT);

    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    let list_body: serde_json::Value = test::read_body_json(list_resp).await;
    assert_eq!(list_body.as_array().unwrap().len(), 0);
}

// ── accept invite + members ───────────────────────────────────────────────

#[tokio::test]
async fn full_invite_accept_remove_cycle() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "cycleowner@example.com", "password123");
    create_verified_user!(&env.pool, "cycleinvitee@example.com", "password123");
    let app = make_app!(env);
    let owner_token = login_token!(&app, "cycleowner@example.com", "password123");
    let invitee_token = login_token!(&app, "cycleinvitee@example.com", "password123");
    let doc_id = create_doc!(&app, &owner_token);

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Editor,
                expires_at: None,
                max_uses: None,
            })
            .to_request(),
    )
    .await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let inv_token = create_body["token"].as_str().unwrap().to_string();

    let accept_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/invites/{}/accept", inv_token))
            .insert_header(("Authorization", format!("Bearer {}", invitee_token)))
            .to_request(),
    )
    .await;
    assert_eq!(accept_resp.status(), StatusCode::OK);
    let accept_body: serde_json::Value = test::read_body_json(accept_resp).await;
    assert_eq!(accept_body["role"], "editor");

    let members_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/members", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .to_request(),
    )
    .await;
    assert_eq!(members_resp.status(), StatusCode::OK);
    let members_body: serde_json::Value = test::read_body_json(members_resp).await;
    assert_eq!(members_body.as_array().unwrap().len(), 1);
    let member_user_id = members_body[0]["user_id"].as_str().unwrap();

    let remove_resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}/members/{}", doc_id, member_user_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .to_request(),
    )
    .await;
    assert_eq!(remove_resp.status(), StatusCode::NO_CONTENT);

    let members_after_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/documents/{}/members", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .to_request(),
    )
    .await;
    let members_after_body: serde_json::Value = test::read_body_json(members_after_resp).await;
    assert_eq!(members_after_body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn accept_invite_revoked_returns_410() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "revokeown@example.com", "password123");
    create_verified_user!(&env.pool, "revokeinvitee@example.com", "password123");
    let app = make_app!(env);
    let owner_token = login_token!(&app, "revokeown@example.com", "password123");
    let invitee_token = login_token!(&app, "revokeinvitee@example.com", "password123");
    let doc_id = create_doc!(&app, &owner_token);

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Viewer,
                expires_at: None,
                max_uses: None,
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(create_resp).await;
    let inv_token = body["token"].as_str().unwrap().to_string();

    test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}/invites/{}", doc_id, inv_token))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .to_request(),
    )
    .await;

    let accept_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/invites/{}/accept", inv_token))
            .insert_header(("Authorization", format!("Bearer {}", invitee_token)))
            .to_request(),
    )
    .await;
    assert_eq!(accept_resp.status(), StatusCode::GONE);
}

#[tokio::test]
async fn accept_invite_max_uses_exhausted_returns_410() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "maxown@example.com", "password123");
    create_verified_user!(&env.pool, "maxuser1@example.com", "password123");
    create_verified_user!(&env.pool, "maxuser2@example.com", "password123");
    let app = make_app!(env);
    let owner_token = login_token!(&app, "maxown@example.com", "password123");
    let user1_token = login_token!(&app, "maxuser1@example.com", "password123");
    let user2_token = login_token!(&app, "maxuser2@example.com", "password123");
    let doc_id = create_doc!(&app, &owner_token);

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Editor,
                expires_at: None,
                max_uses: Some(1),
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(create_resp).await;
    let inv_token = body["token"].as_str().unwrap().to_string();

    let first = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/invites/{}/accept", inv_token))
            .insert_header(("Authorization", format!("Bearer {}", user1_token)))
            .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/invites/{}/accept", inv_token))
            .insert_header(("Authorization", format!("Bearer {}", user2_token)))
            .to_request(),
    )
    .await;
    assert_eq!(second.status(), StatusCode::GONE);
}

#[tokio::test]
async fn owner_cannot_remove_self() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "selfremove@example.com", "password123");
    let owner_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("selfremove@example.com")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    let app = make_app!(env);
    let token = login_token!(&app, "selfremove@example.com", "password123");
    let doc_id = create_doc!(&app, &token);

    let resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/documents/{}/members/{}", doc_id, owner_id.0))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_member_role_changes_role() {
    let env = TestEnv::new().await;
    create_verified_user!(&env.pool, "roleown@example.com", "password123");
    create_verified_user!(&env.pool, "roleinvitee@example.com", "password123");
    let app = make_app!(env);
    let owner_token = login_token!(&app, "roleown@example.com", "password123");
    let invitee_token = login_token!(&app, "roleinvitee@example.com", "password123");
    let doc_id = create_doc!(&app, &owner_token);

    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/documents/{}/invites", doc_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(CreateInviteLinkRequest {
                role: MemberRole::Editor,
                expires_at: None,
                max_uses: None,
            })
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(create_resp).await;
    let inv_token = body["token"].as_str().unwrap().to_string();

    let accept_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/invites/{}/accept", inv_token))
            .insert_header(("Authorization", format!("Bearer {}", invitee_token)))
            .to_request(),
    )
    .await;
    let accept_body: serde_json::Value = test::read_body_json(accept_resp).await;
    let member_user_id = accept_body["user_id"].as_str().unwrap().to_string();

    let patch_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/documents/{}/members/{}", doc_id, member_user_id))
            .insert_header(("Authorization", format!("Bearer {}", owner_token)))
            .set_json(UpdateMemberRoleRequest {
                role: MemberRole::Viewer,
            })
            .to_request(),
    )
    .await;
    assert_eq!(patch_resp.status(), StatusCode::OK);
    let patch_body: serde_json::Value = test::read_body_json(patch_resp).await;
    assert_eq!(patch_body["role"], "viewer");
}
