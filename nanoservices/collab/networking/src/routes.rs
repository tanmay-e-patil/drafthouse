use actix_web::web;
use collab_core::DocStore;
use dal::{ScyllaDescriptor, postgres_txs::SqlxPostGresDescriptor};

use crate::handlers;

pub fn configure(
    cfg: &mut web::ServiceConfig,
    pg_dal: web::Data<SqlxPostGresDescriptor>,
    scylla_dal: web::Data<ScyllaDescriptor>,
    doc_store: web::Data<DocStore>,
) {
    cfg.app_data(scylla_dal)
        .app_data(doc_store)
        .service(web::scope("/collab").route("/{doc_id}", web::get().to(handlers::ws_handler)));
    let _ = pg_dal; // already registered by documents-networking
}
