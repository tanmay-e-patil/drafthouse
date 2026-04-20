use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware as actix_middleware, web};
use dal::postgres_txs::SqlxPostGresDescriptor;
use sqlx::PgPool;
use std::env;
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
    tracing::info!("Connected to database");

    let dal = web::Data::new(SqlxPostGresDescriptor { pool });

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
            .configure(|cfg| auth_networking::routes::configure(cfg, dal.clone()))
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}
