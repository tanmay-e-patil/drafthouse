use actix_cors::Cors;
use actix_web::{
    App, HttpServer,
    dev::{Service, ServiceRequest},
    http::{Method, header},
    web,
};
use collab_core::DocStore;
use dal::{ScyllaDescriptor, postgres_txs::SqlxPostGresDescriptor};
use dashmap::DashMap;
use sqlx::PgPool;
use std::{collections::HashSet, env, sync::Arc};
use tokio::time::{Duration, interval};
use tracing_actix_web::{DefaultRootSpanBuilder, RootSpanBuilder, TracingLogger};
use tracing_subscriber::EnvFilter;

const DEFAULT_APP_ORIGIN: &str = "http://localhost:3000";

struct AppRootSpanBuilder;

impl RootSpanBuilder for AppRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> tracing::Span {
        let span = DefaultRootSpanBuilder::on_request_start(request);

        if let Some(user_id) = request
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .and_then(|token| auth_core::jwt::verify_jwt(token).ok())
            .map(|claims| claims.sub.to_string())
        {
            span.record("user_id", user_id);
        }

        span
    }

    fn on_request_end<B: actix_web::body::MessageBody>(
        span: tracing::Span,
        outcome: &Result<actix_web::dev::ServiceResponse<B>, actix_web::Error>,
    ) {
        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}

fn normalize_origin(origin: &str) -> Option<String> {
    let trimmed = origin.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let uri = trimmed.parse::<actix_web::http::Uri>().ok()?;
    let scheme = uri.scheme_str()?;
    let authority = uri.authority()?;
    Some(format!("{scheme}://{authority}"))
}

fn parse_allowed_origins(raw: &str) -> Vec<String> {
    raw.split(',').filter_map(normalize_origin).collect()
}

fn is_origin_allowed(origin: &str, allowed_origins: &HashSet<String>) -> bool {
    normalize_origin(origin).is_some_and(|origin| allowed_origins.contains(&origin))
}

fn header_to_string(value: Option<&header::HeaderValue>) -> Option<String> {
    value
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn load_allowed_origins() -> Vec<String> {
    match env::var("APP_ORIGIN") {
        Ok(raw) => {
            let origins = parse_allowed_origins(&raw);
            if origins.is_empty() {
                let fallback = vec![DEFAULT_APP_ORIGIN.to_string()];
                tracing::warn!(
                    raw = %raw,
                    fallback = ?fallback,
                    "APP_ORIGIN was empty or invalid; falling back to default CORS origin"
                );
                fallback
            } else {
                tracing::info!(raw = %raw, origins = ?origins, "Configured CORS allowlist");
                origins
            }
        }
        Err(_) => {
            let fallback = vec![DEFAULT_APP_ORIGIN.to_string()];
            tracing::warn!(
                fallback = ?fallback,
                "APP_ORIGIN not set; falling back to default CORS origin"
            );
            fallback
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("ingress=debug".parse()?)
                .add_directive("auth_core=debug".parse()?)
                .add_directive("auth_networking=debug".parse()?)
                .add_directive("documents_core=debug".parse()?)
                .add_directive("documents_networking=debug".parse()?)
                .add_directive("collab_core=debug".parse()?)
                .add_directive("collab_networking=debug".parse()?),
        )
        .with_target(true)
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
    let allowed_origins = load_allowed_origins();
    let allowed_origin_set = Arc::new(allowed_origins.iter().cloned().collect::<HashSet<_>>());

    HttpServer::new(move || {
        let cors_allowed_origins = Arc::clone(&allowed_origin_set);
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, req_head| {
                let request_origin = origin.to_str().ok().map(str::to_owned);
                let normalized_origin = request_origin.as_deref().and_then(normalize_origin);
                let access_control_request_method = header_to_string(
                    req_head
                        .headers()
                        .get(header::ACCESS_CONTROL_REQUEST_METHOD),
                );
                let access_control_request_headers = header_to_string(
                    req_head
                        .headers()
                        .get(header::ACCESS_CONTROL_REQUEST_HEADERS),
                );
                let allowed = request_origin
                    .as_deref()
                    .is_some_and(|origin| is_origin_allowed(origin, &cors_allowed_origins));

                if allowed {
                    tracing::debug!(
                        method = %req_head.method,
                        path = %req_head.uri.path(),
                        request_origin = %request_origin.as_deref().unwrap_or("<non-utf8>"),
                        normalized_origin = %normalized_origin.as_deref().unwrap_or("<invalid>"),
                        access_control_request_method = ?access_control_request_method,
                        access_control_request_headers = ?access_control_request_headers,
                        "CORS origin accepted"
                    );
                } else {
                    tracing::warn!(
                        method = %req_head.method,
                        path = %req_head.uri.path(),
                        request_origin = %request_origin.as_deref().unwrap_or("<non-utf8>"),
                        normalized_origin = %normalized_origin.as_deref().unwrap_or("<invalid>"),
                        access_control_request_method = ?access_control_request_method,
                        access_control_request_headers = ?access_control_request_headers,
                        allowed_origins = ?cors_allowed_origins.as_ref(),
                        "CORS origin rejected"
                    );
                }

                allowed
            })
            .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                header::ACCEPT,
            ])
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(TracingLogger::<AppRootSpanBuilder>::new())
            .wrap_fn(|req, srv| {
                let method = req.method().clone();
                let path = req.path().to_string();
                let origin = header_to_string(req.headers().get(header::ORIGIN));
                let access_control_request_method =
                    header_to_string(req.headers().get(header::ACCESS_CONTROL_REQUEST_METHOD));
                let access_control_request_headers =
                    header_to_string(req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS));
                let is_preflight = method == Method::OPTIONS
                    && origin.is_some()
                    && access_control_request_method.is_some();

                let fut = srv.call(req);

                async move {
                    let res = fut.await?;

                    if is_preflight {
                        let access_control_allow_origin = header_to_string(
                            res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
                        );
                        tracing::info!(
                            method = %method,
                            path = %path,
                            origin = %origin.as_deref().unwrap_or("<missing>"),
                            access_control_request_method = ?access_control_request_method,
                            access_control_request_headers = ?access_control_request_headers,
                            status = %res.status(),
                            access_control_allow_origin = ?access_control_allow_origin,
                            "CORS preflight processed"
                        );
                    }

                    Ok(res)
                }
            })
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{DEFAULT_APP_ORIGIN, is_origin_allowed, parse_allowed_origins};

    #[test]
    fn parses_comma_separated_origins() {
        assert_eq!(
            parse_allowed_origins(" https://drafthouse.tanmayep.dev, https://drafthouse.app/ "),
            vec![
                "https://drafthouse.tanmayep.dev".to_string(),
                "https://drafthouse.app".to_string(),
            ]
        );
    }

    #[test]
    fn strips_paths_and_trailing_slashes() {
        assert_eq!(
            parse_allowed_origins("https://drafthouse.tanmayep.dev/register/"),
            vec!["https://drafthouse.tanmayep.dev".to_string()]
        );
    }

    #[test]
    fn ignores_blank_entries() {
        assert!(parse_allowed_origins(" ,  , ").is_empty());
        assert_eq!(
            parse_allowed_origins(&format!("{DEFAULT_APP_ORIGIN}/")),
            vec![DEFAULT_APP_ORIGIN.to_string()]
        );
    }

    #[test]
    fn matches_normalized_origin_against_allowlist() {
        let allowed_origins = HashSet::from_iter(["https://drafthouse.tanmayep.dev".to_string()]);

        assert!(is_origin_allowed(
            "https://drafthouse.tanmayep.dev/",
            &allowed_origins
        ));
        assert!(is_origin_allowed(
            "https://drafthouse.tanmayep.dev/register",
            &allowed_origins
        ));
        assert!(!is_origin_allowed(
            "https://drafthouse.app",
            &allowed_origins
        ));
    }
}
