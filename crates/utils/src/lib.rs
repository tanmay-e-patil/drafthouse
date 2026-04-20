use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NanoServiceErrorStatus {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    InternalServerError,
}

impl From<NanoServiceErrorStatus> for u16 {
    fn from(val: NanoServiceErrorStatus) -> u16 {
        match val {
            NanoServiceErrorStatus::BadRequest => 400,
            NanoServiceErrorStatus::Unauthorized => 401,
            NanoServiceErrorStatus::Forbidden => 403,
            NanoServiceErrorStatus::NotFound => 404,
            NanoServiceErrorStatus::Conflict => 409,
            NanoServiceErrorStatus::InternalServerError => 500,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub struct NanoServiceError {
    pub status: NanoServiceErrorStatus,
    pub message: String,
}

impl NanoServiceError {
    pub fn new(message: impl Into<String>, status: NanoServiceErrorStatus) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for NanoServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", u16::from(self.status), self.message)
    }
}

#[derive(Serialize)]
pub struct ProblemDetail {
    pub type_: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
}

impl ResponseError for NanoServiceError {
    fn error_response(&self) -> HttpResponse {
        let status_code = u16::from(self.status);
        let title = match self.status {
            NanoServiceErrorStatus::BadRequest => "Bad Request",
            NanoServiceErrorStatus::Unauthorized => "Unauthorized",
            NanoServiceErrorStatus::Forbidden => "Forbidden",
            NanoServiceErrorStatus::NotFound => "Not Found",
            NanoServiceErrorStatus::Conflict => "Conflict",
            NanoServiceErrorStatus::InternalServerError => "Internal Server Error",
        };

        let body = ProblemDetail {
            type_: format!("https://drafthouse.app/errors/{}", self.status_str()),
            title: title.to_string(),
            status: status_code,
            detail: self.message.clone(),
        };

        HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap()).json(body)
    }
}

impl NanoServiceError {
    fn status_str(&self) -> &'static str {
        match self.status {
            NanoServiceErrorStatus::BadRequest => "bad-request",
            NanoServiceErrorStatus::Unauthorized => "unauthorized",
            NanoServiceErrorStatus::Forbidden => "forbidden",
            NanoServiceErrorStatus::NotFound => "not-found",
            NanoServiceErrorStatus::Conflict => "conflict",
            NanoServiceErrorStatus::InternalServerError => "internal-server-error",
        }
    }
}

#[macro_export]
macro_rules! safe_eject {
    ($e:expr, $err_status:expr) => {
        $e.map_err(|x| $crate::errors::NanoServiceError::new(x.to_string(), $err_status))
    };
    ($e:expr, $err_status:expr, $message_context:expr) => {
        $e.map_err(|x| {
            $crate::errors::NanoServiceError::new(
                format!("{}: {}", $message_context, x.to_string()),
                $err_status,
            )
        })
    };
}

pub mod errors {
    pub use super::*;
}
