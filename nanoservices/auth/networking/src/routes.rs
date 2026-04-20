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
            )
            .route("/login", web::post().to(handlers::login))
            .route("/refresh", web::post().to(handlers::refresh))
            .route("/logout", web::post().to(handlers::logout))
            .route("/logout-all", web::post().to(handlers::logout_all))
            .route(
                "/forgot-password",
                web::post().to(handlers::forgot_password),
            )
            .route(
                "/reset-password",
                web::post().to(handlers::reset_password),
            ),
    );
}
