use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub welcome_doc_created: bool,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailVerificationToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

pub struct NewEmailVerificationToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResendVerificationRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegisterResponse {
    pub user_id: Uuid,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyEmailResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResendVerificationResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: Uuid,
    pub email: String,
    pub verified: bool,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

pub struct NewRefreshToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub welcome_doc_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PasswordResetToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

pub struct NewPasswordResetToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgotPasswordResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetPasswordResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewDocument {
    pub owner_id: Uuid,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentListResponse {
    pub data: Vec<Document>,
    pub next_cursor: Option<Uuid>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentPresencePeer {
    pub name: String,
    pub color: String,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentPresenceResponse {
    pub data: Vec<DocumentPresencePeer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "member_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Editor,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentMember {
    pub doc_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InviteLink {
    pub token: String,
    pub doc_id: Uuid,
    pub role: MemberRole,
    pub created_by: Uuid,
    pub max_uses: Option<i32>,
    pub use_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub struct NewInviteLink {
    pub token: String,
    pub doc_id: Uuid,
    pub role: MemberRole,
    pub created_by: Uuid,
    pub max_uses: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteLinkRequest {
    pub role: MemberRole,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: MemberRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContentResponse {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentContentRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WsTicket {
    pub token_hash: String,
    pub doc_id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub struct NewWsTicket {
    pub token_hash: String,
    pub doc_id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsTicketResponse {
    pub ticket: String,
}

#[derive(Debug, Clone)]
pub struct CollabOp {
    pub doc_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub op_id: Uuid,
    pub client_id: Uuid,
    pub data: Vec<u8>,
}

pub struct NewCollabOp {
    pub doc_id: Uuid,
    pub op_id: Uuid,
    pub client_id: Uuid,
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CollabSnapshot {
    pub doc_id: Uuid,
    pub version: i32,
    pub data: Vec<u8>,
    pub checksum: String,
    pub taken_at: DateTime<Utc>,
}

pub struct NewCollabSnapshot {
    pub doc_id: Uuid,
    pub version: i32,
    pub data: Vec<u8>,
    pub checksum: String,
    pub taken_at: DateTime<Utc>,
}

/// In-process event: published by documents service after a title change,
/// consumed by collab service to broadcast `title_update` to all WS clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleUpdated {
    pub doc_id: Uuid,
    pub title: String,
}
