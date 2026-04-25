use anyhow::{Context, Result};
use scylla::{Session, SessionBuilder};
use std::{env, fs, path::PathBuf};
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<()> {
    let nodes = env::var("SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".into());
    let keyspace = env::var("SCYLLA_KEYSPACE").unwrap_or_else(|_| "drafthouse".into());
    let migrations_dir =
        env::var("SCYLLA_MIGRATIONS_DIR").unwrap_or_else(|_| "migrations/scylla".into());

    let session: Session = connect_with_retry(&nodes).await?;

    bootstrap_tracking(&session, &keyspace).await?;

    let mut paths: Vec<PathBuf> = fs::read_dir(&migrations_dir)
        .context("read migrations/scylla")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cql"))
        .collect();
    paths.sort();

    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if is_applied(&session, &keyspace, &name).await? {
            println!("skip  {name}");
            continue;
        }
        let cql = fs::read_to_string(&path).with_context(|| format!("read {name}"))?;
        for stmt in cql
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with("--"))
        {
            session
                .query_unpaged(stmt, &[])
                .await
                .with_context(|| format!("execute statement in {name}"))?;
        }
        record_applied(&session, &keyspace, &name).await?;
        println!("apply {name}");
    }

    println!("Scylla migrations complete.");
    Ok(())
}

async fn connect_with_retry(nodes: &str) -> Result<Session> {
    const MAX_ATTEMPTS: usize = 30;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    let mut last_err = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match SessionBuilder::new().known_node(nodes).build().await {
            Ok(session) => return Ok(session),
            Err(err) if attempt < MAX_ATTEMPTS => {
                eprintln!(
                    "Scylla connection attempt {attempt}/{MAX_ATTEMPTS} failed: {err}. Retrying in {}s.",
                    RETRY_DELAY.as_secs()
                );
                last_err = Some(err);
                sleep(RETRY_DELAY).await;
            }
            Err(err) => return Err(err).context("connect to ScyllaDB"),
        }
    }

    Err(last_err
        .context("connect to ScyllaDB after retries")
        .unwrap_err())
}

async fn bootstrap_tracking(session: &Session, keyspace: &str) -> Result<()> {
    let rf = env::var("SCYLLA_REPLICATION_FACTOR").unwrap_or_else(|_| "1".into());
    session
        .query_unpaged(
            format!(
                "CREATE KEYSPACE IF NOT EXISTS {keyspace} \
                 WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': {rf}}}"
            ),
            &[],
        )
        .await
        .context("create keyspace")?;

    session
        .query_unpaged(
            format!(
                "CREATE TABLE IF NOT EXISTS {keyspace}.schema_migrations \
                 (name TEXT PRIMARY KEY, applied_at TIMESTAMP)"
            ),
            &[],
        )
        .await
        .context("create schema_migrations table")?;

    Ok(())
}

async fn is_applied(session: &Session, keyspace: &str, name: &str) -> Result<bool> {
    let rows = session
        .query_unpaged(
            format!("SELECT name FROM {keyspace}.schema_migrations WHERE name = ?"),
            (name,),
        )
        .await
        .context("query schema_migrations")?;
    Ok(rows
        .into_rows_result()?
        .rows::<(String,)>()?
        .next()
        .is_some())
}

async fn record_applied(session: &Session, keyspace: &str, name: &str) -> Result<()> {
    session
        .query_unpaged(
            format!(
                "INSERT INTO {keyspace}.schema_migrations \
                 (name, applied_at) VALUES (?, toTimestamp(now()))"
            ),
            (name,),
        )
        .await
        .context("record migration")?;
    Ok(())
}
