use actix_web::web;
use dal::postgres_txs::SqlxPostGresDescriptor;

use crate::handlers;

pub fn configure(cfg: &mut web::ServiceConfig, dal: web::Data<SqlxPostGresDescriptor>) {
    cfg.app_data(dal).service(
        web::scope("/documents")
            .route("", web::post().to(handlers::create_document))
            .route("", web::get().to(handlers::list_documents))
            .route("/{id}", web::get().to(handlers::get_document))
            .route("/{id}", web::patch().to(handlers::update_document))
            .route("/{id}", web::delete().to(handlers::delete_document))
            .route(
                "/{id}/content",
                web::get().to(handlers::get_document_content),
            )
            .route(
                "/{id}/content",
                web::patch().to(handlers::update_document_content),
            ),
    );
}
