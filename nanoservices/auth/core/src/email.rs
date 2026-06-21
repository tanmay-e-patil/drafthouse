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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<ResendEmailAttachment>,
}

#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_base64: String,
}

#[derive(Serialize)]
struct ResendEmailAttachment {
    filename: String,
    content: String,
}

pub async fn send_verification_email(to_email: &str, token: &str) -> Result<(), NanoServiceError> {
    let api_key = env::var("RESEND_API_KEY").unwrap_or_else(|_| "re_test_key".into());
    let app_origin = env::var("APP_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());

    let verify_url = format!("{}/verify-email?token={}", app_origin, token);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"></head>
<body style="margin:0;background:#f7f1df;color:#241f18;font-family:Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <div style="display:none;max-height:0;overflow:hidden;">Confirm your Drafthouse email to open your private writing room.</div>
  <main style="padding:32px 16px;background:radial-gradient(circle at 15% 18%,#f2cf87 0,#f2cf8700 34%),radial-gradient(circle at 82% 6%,#b8db9b 0,#b8db9b00 30%),linear-gradient(135deg,#fffaf0,#f3ead2);">
    <section style="max-width:560px;margin:0 auto;border:1px solid #ddd0b2;border-radius:24px;background:#fffaf0cc;box-shadow:0 18px 60px #6b4f1d24;overflow:hidden;">
      <div style="padding:28px 28px 18px;">
        <div style="display:inline-flex;align-items:center;gap:8px;margin-bottom:24px;font-weight:700;letter-spacing:-0.02em;">
          <span style="display:inline-grid;place-items:center;width:34px;height:34px;border-radius:12px;background:#328f97;color:#fff;">D</span>
          Drafthouse
        </div>
        <p style="display:inline-block;margin:0 0 14px;padding:5px 10px;border:1px solid #328f9733;border-radius:999px;background:#ffffff99;color:#6d6252;font-size:12px;font-weight:600;">Verified writers only</p>
        <h1 style="margin:0;font-size:30px;line-height:1.12;letter-spacing:-0.04em;">Confirm your email to start drafting.</h1>
        <p style="margin:16px 0 24px;color:#6d6252;font-size:15px;line-height:1.7;">One click opens private Markdown workspaces, live collaboration, and secure sharing for your Drafthouse account.</p>
        <a href="{verify_url}" style="display:inline-block;border-radius:12px;background:#328f97;color:#fff;padding:13px 20px;text-decoration:none;font-size:14px;font-weight:700;box-shadow:0 8px 22px #328f9740;">Confirm email</a>
        <p style="margin:22px 0 0;color:#6d6252;font-size:13px;line-height:1.6;">This link expires in 24 hours. If the button does not work, copy and paste this URL into your browser:</p>
        <p style="word-break:break-all;margin:8px 0 0;font-size:12px;line-height:1.6;"><a href="{verify_url}" style="color:#28777e;">{verify_url}</a></p>
      </div>
      <div style="border-top:1px solid #e4d8bd;padding:16px 28px;background:#fff7e8;color:#7a6f5f;font-size:12px;line-height:1.6;">If you did not create a Drafthouse account, you can ignore this email.</div>
    </section>
  </main>
</body>
</html>"#
    );

    let from =
        env::var("EMAIL_FROM").unwrap_or_else(|_| "Drafthouse <onboarding@tanmayep.dev>".into());

    let payload = ResendEmailPayload {
        from,
        to: vec![to_email.to_string()],
        subject: "Confirm your Drafthouse email".to_string(),
        html,
        attachments: vec![],
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
        attachments: vec![],
    };

    send_email(payload, &api_key, "Failed to send password reset email").await?;

    tracing::info!(email = %to_email, "Password reset email sent");
    Ok(())
}

pub async fn send_export_email(
    to_email: &str,
    attachment: &EmailAttachment,
) -> Result<(), NanoServiceError> {
    let api_key = env::var("RESEND_API_KEY").unwrap_or_else(|_| "re_test_key".into());
    let from =
        env::var("EMAIL_FROM").unwrap_or_else(|_| "Drafthouse <onboarding@tanmayep.dev>".into());
    let generated_at = chrono::Utc::now().to_rfc3339();
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
  <h2>Your Drafthouse export is ready</h2>
  <p>Your export is attached to this email as a ZIP archive of your Markdown documents.</p>
  <p>Generated at: {generated_at}</p>
</body>
</html>"#
    );

    let base_url =
        env::var("RESEND_API_BASE_URL").unwrap_or_else(|_| "https://api.resend.com".into());

    let _ = base_url;

    let payload = ResendEmailPayload {
        from,
        to: vec![to_email.to_string()],
        subject: "Your Drafthouse export".to_string(),
        html,
        attachments: vec![ResendEmailAttachment {
            filename: attachment.filename.clone(),
            content: attachment.content_base64.clone(),
        }],
    };

    send_email(payload, &api_key, "Failed to send export email").await?;
    tracing::info!(email = %to_email, "Export email sent");
    Ok(())
}

async fn send_email(
    payload: ResendEmailPayload,
    api_key: &str,
    error_message: &str,
) -> Result<(), NanoServiceError> {
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
            error_message.to_string(),
            NanoServiceErrorStatus::InternalServerError,
        ));
    }

    tracing::info!("Password reset email sent");
    Ok(())
}
