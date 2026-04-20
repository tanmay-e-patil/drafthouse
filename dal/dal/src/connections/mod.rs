use sqlx::PgPool;
use std::env;

pub struct AppState {
    pub pool: PgPool,
}

impl AppState {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://drafthouse:drafthouse@localhost:5432/drafthouse".into()
        });
        let pool = PgPool::connect(&database_url).await?;
        Ok(Self { pool })
    }
}
