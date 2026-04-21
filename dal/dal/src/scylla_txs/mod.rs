pub mod collab;

use scylla::{Session, SessionBuilder};
use std::{env, sync::Arc};

#[derive(Clone)]
pub struct ScyllaDescriptor {
    pub session: Arc<Session>,
    pub keyspace: String,
}

impl ScyllaDescriptor {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let nodes = env::var("SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".into());
        let keyspace = env::var("SCYLLA_KEYSPACE").unwrap_or_else(|_| "drafthouse".into());
        let session = SessionBuilder::new().known_node(&nodes).build().await?;
        Ok(Self {
            session: Arc::new(session),
            keyspace,
        })
    }
}
