use actix_web::web;
use dal::postgres_txs::SqlxPostGresDescriptor;

use crate::handlers;

pub fn configure(cfg: &mut web::ServiceConfig, dal: web::Data<SqlxPostGresDescriptor>) {
    cfg.app_data(dal).service(
        web::scope("/auth")
            .route("/register", web::post().to(handlers::register))
            .route("/verify-email", web::post().to(handlers::verify_email))
            .route(
                "/resend-verification",
                web::post().to(handlers::resend_verification),
            ),
    );
}
