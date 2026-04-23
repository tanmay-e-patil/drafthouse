use actix_web::web;
use collab_core::DocStore;
use dal::postgres_txs::SqlxPostGresDescriptor;

use crate::handlers;

pub fn configure(
    cfg: &mut web::ServiceConfig,
    dal: web::Data<SqlxPostGresDescriptor>,
    doc_store: web::Data<DocStore>,
) {
    cfg.app_data(dal)
        .app_data(doc_store)
        .service(
            web::scope("/documents")
                .route("", web::post().to(handlers::create_document))
                .route("", web::get().to(handlers::list_documents))
                .route("/{id}", web::get().to(handlers::get_document))
                .route("/{id}", web::patch().to(handlers::update_document))
                .route("/{id}", web::delete().to(handlers::delete_document))
                .route(
                    "/{id}/presence",
                    web::get().to(handlers::get_document_presence),
                )
                .route(
                    "/{id}/content",
                    web::get().to(handlers::get_document_content),
                )
                .route(
                    "/{id}/content",
                    web::patch().to(handlers::update_document_content),
                )
                .route("/{id}/ws-ticket", web::post().to(handlers::issue_ws_ticket))
                .route(
                    "/{id}/invites",
                    web::post().to(handlers::create_invite_link),
                )
                .route("/{id}/invites", web::get().to(handlers::list_invite_links))
                .route(
                    "/{id}/invites/{token}",
                    web::delete().to(handlers::revoke_invite_link),
                )
                .route("/{id}/members", web::get().to(handlers::list_members))
                .route(
                    "/{id}/members/{uid}",
                    web::delete().to(handlers::remove_member),
                )
                .route(
                    "/{id}/members/{uid}",
                    web::patch().to(handlers::update_member_role),
                ),
        )
        .service(
            web::scope("/invites")
                .route("/{token}/accept", web::post().to(handlers::accept_invite)),
        );
}
