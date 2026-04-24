use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let migrations_dir =
        env::var("MIGRATIONS_DIR").unwrap_or_else(|_| "migrations/postgres".into());

    let pool: PgPool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    sqlx::migrate::Migrator::new(std::path::Path::new(&migrations_dir))
        .await?
        .run(&pool)
        .await?;

    println!("Postgres migrations complete.");
    Ok(())
}
