use serde::Serialize;
use std::env;
use tracing;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

#[derive(Serialize)]
struct ResendEmailPayload {
    from: String,
    to: Vec<String>,
    subject: String,
    html: String,
}

pub async fn send_verification_email(to_email: &str, token: &str) -> Result<(), NanoServiceError> {
    let api_key = env::var("RESEND_API_KEY").unwrap_or_else(|_| "re_test_key".into());
    let app_origin = env::var("APP_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());

    let verify_url = format!("{}/verify-email?token={}", app_origin, token);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
  <h2>Welcome to Drafthouse</h2>
  <p>Please verify your email address by clicking the link below:</p>
  <p><a href="{verify_url}" style="background-color: #328f97; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; display: inline-block;">Verify Email</a></p>
  <p>This link will expire in 24 hours.</p>
  <p>If you didn't create an account, you can safely ignore this email.</p>
</body>
</html>"#
    );

    let from =
        env::var("EMAIL_FROM").unwrap_or_else(|_| "Drafthouse <onboarding@tanmayep.dev>".into());

    let payload = ResendEmailPayload {
        from,
        to: vec![to_email.to_string()],
        subject: "Verify your email - Drafthouse".to_string(),
        html,
    };

    let base_url =
        env::var("RESEND_API_BASE_URL").unwrap_or_else(|_| "https://api.resend.com".into());

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/emails", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            NanoServiceError::new(
                format!("Failed to send email: {}", e),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::error!("Resend API error {}: {}", status, body);
        return Err(NanoServiceError::new(
            "Failed to send verification email".to_string(),
            NanoServiceErrorStatus::InternalServerError,
        ));
    }

    tracing::info!(email = %to_email, "Verification email sent");
    Ok(())
}

pub async fn send_password_reset_email(
    to_email: &str,
    token: &str,
) -> Result<(), NanoServiceError> {
    let api_key = env::var("RESEND_API_KEY").unwrap_or_else(|_| "re_test_key".into());
    let app_origin = env::var("APP_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());

    let reset_url = format!("{}/reset-password?token={}", app_origin, token);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
  <h2>Reset your password</h2>
  <p>You requested a password reset. Click the link below to set a new password:</p>
  <p><a href="{reset_url}" style="background-color: #328f97; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; display: inline-block;">Reset Password</a></p>
  <p>This link will expire in 15 minutes.</p>
  <p>If you didn't request a password reset, you can safely ignore this email.</p>
</body>
</html>"#
    );

    let from =
        env::var("EMAIL_FROM").unwrap_or_else(|_| "Drafthouse <onboarding@tanmayep.dev>".into());

    let payload = ResendEmailPayload {
        from,
        to: vec![to_email.to_string()],
        subject: "Reset your password - Drafthouse".to_string(),
        html,
    };

    let base_url =
        env::var("RESEND_API_BASE_URL").unwrap_or_else(|_| "https://api.resend.com".into());

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/emails", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            NanoServiceError::new(
                format!("Failed to send email: {}", e),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::error!("Resend API error {}: {}", status, body);
        return Err(NanoServiceError::new(
            "Failed to send password reset email".to_string(),
            NanoServiceErrorStatus::InternalServerError,
        ));
    }

    tracing::info!(email = %to_email, "Password reset email sent");
    Ok(())
}
