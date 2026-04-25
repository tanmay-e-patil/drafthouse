pub mod collab;

use scylla::{Session, SessionBuilder};
use std::{env, sync::Arc};
use tokio::time::{Duration, sleep};

#[derive(Clone)]
pub struct ScyllaDescriptor {
    pub session: Arc<Session>,
    pub keyspace: String,
}

impl ScyllaDescriptor {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let nodes = env::var("SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".into());
        let keyspace = env::var("SCYLLA_KEYSPACE").unwrap_or_else(|_| "drafthouse".into());
        let session = connect_with_retry(&nodes).await?;
        Ok(Self {
            session: Arc::new(session),
            keyspace,
        })
    }
}

async fn connect_with_retry(nodes: &str) -> Result<Session, Box<dyn std::error::Error>> {
    const MAX_ATTEMPTS: usize = 30;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    let mut last_err: Option<Box<dyn std::error::Error>> = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match SessionBuilder::new().known_node(nodes).build().await {
            Ok(session) => return Ok(session),
            Err(err) if attempt < MAX_ATTEMPTS => {
                eprintln!(
                    "Scylla connection attempt {attempt}/{MAX_ATTEMPTS} failed: {err}. Retrying in {}s.",
                    RETRY_DELAY.as_secs()
                );
                last_err = Some(Box::new(err));
                sleep(RETRY_DELAY).await;
            }
            Err(err) => return Err(Box::new(err)),
        }
    }

    Err(last_err.unwrap_or_else(|| "failed to connect to Scylla".into()))
}
