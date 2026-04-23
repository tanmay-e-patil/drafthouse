use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware as actix_middleware, web};
use collab_core::DocStore;
use dal::{ScyllaDescriptor, postgres_txs::SqlxPostGresDescriptor};
use dashmap::DashMap;
use sqlx::PgPool;
use std::env;
use tokio::time::{Duration, interval};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("drafthouse=debug".parse()?))
        .with_target(false)
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://drafthouse:drafthouse@localhost:5432/drafthouse".into());

    let pool = PgPool::connect(&database_url).await?;
    tracing::info!("Connected to Postgres");

    let pg_dal = web::Data::new(SqlxPostGresDescriptor { pool });

    let scylla_dal = web::Data::new(ScyllaDescriptor::new().await?);
    tracing::info!("Connected to ScyllaDB");

    let doc_store: web::Data<DocStore> = web::Data::new(DashMap::new());

    // Expose DocStore to the collab title-sync event subscriber
    collab_core::init_doc_store(doc_store.clone().into_inner());

    // Background eviction sweep every 60s
    {
        let store = doc_store.clone();
        let dal = scylla_dal.get_ref().clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(collab_core::room::EVICTION_SWEEP_SECS));
            loop {
                ticker.tick().await;
                collab_core::snapshot::eviction_sweep(&dal, &store).await;
            }
        });
    }

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::ACCEPT,
            ])
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(actix_middleware::Logger::default())
            .wrap(cors)
            .configure(|cfg| auth_networking::routes::configure(cfg, pg_dal.clone()))
            .configure(|cfg| {
                documents_networking::routes::configure(cfg, pg_dal.clone(), doc_store.clone())
            })
            .configure(|cfg| {
                collab_networking::routes::configure(
                    cfg,
                    pg_dal.clone(),
                    scylla_dal.clone(),
                    doc_store.clone(),
                )
            })
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}
