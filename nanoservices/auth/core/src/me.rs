use std::{
    collections::{HashMap, HashSet},
    io::Write,
    sync::{LazyLock, Mutex},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dal::{
    DeleteAllRefreshTokensForUser, DeleteUser, GetDocumentContent, GetUserById,
    ListDocumentsByOwnerNoPagination, postgres_txs::SqlxPostGresDescriptor,
};
use kernel::{
    ChangePasswordResponse, DeleteAccountResponse, Document, ExportRequested, ExportResponse,
    MeResponse,
};
use nan_serve_event_subscriber::subscribe_to_event;
use nan_serve_publish_event::publish_event;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{email, password};

static EXPORT_DALS: LazyLock<Mutex<HashMap<uuid::Uuid, SqlxPostGresDescriptor>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const MIN_PASSWORD_LENGTH: usize = 8;
const MAX_EXPORT_FILENAME_STEM_LEN: usize = 80;

pub fn register_export_dal(user_id: uuid::Uuid, dal: SqlxPostGresDescriptor) {
    EXPORT_DALS
        .lock()
        .expect("export dal mutex poisoned")
        .insert(user_id, dal);
}

pub async fn get_me<D>(dal: &D, user_id: uuid::Uuid) -> Result<MeResponse, NanoServiceError>
where
    D: GetUserById,
{
    let user = load_user(dal, user_id).await?;
    Ok(MeResponse {
        id: user.id,
        email: user.email,
        email_verified_at: user.email_verified_at,
        created_at: user.created_at,
    })
}

pub async fn change_password<D>(
    dal: &D,
    user_id: uuid::Uuid,
    current_password: &str,
    new_password: &str,
) -> Result<ChangePasswordResponse, NanoServiceError>
where
    D: GetUserById + DeleteAllRefreshTokensForUser + dal::UpdateUserPassword,
{
    validate_new_password(new_password)?;

    let user = load_user(dal, user_id).await?;
    ensure_password_matches(current_password, &user.password_hash).await?;

    let new_hash = password::hash_password(new_password)?;
    dal.update_user_password(user.id, new_hash).await?;
    dal.delete_all_refresh_tokens_for_user(user.id).await?;

    Ok(ChangePasswordResponse {
        message: "Password updated successfully.".to_string(),
    })
}

pub async fn delete_account<D>(
    dal: &D,
    user_id: uuid::Uuid,
    current_password: &str,
) -> Result<DeleteAccountResponse, NanoServiceError>
where
    D: GetUserById + DeleteAllRefreshTokensForUser + DeleteUser,
{
    let user = load_user(dal, user_id).await?;
    ensure_password_matches(current_password, &user.password_hash).await?;

    dal.delete_all_refresh_tokens_for_user(user.id).await?;
    dal.delete_user(user.id).await?;

    Ok(DeleteAccountResponse {
        message: "Account deleted successfully.".to_string(),
    })
}

pub async fn request_export<D>(
    dal: &D,
    user_id: uuid::Uuid,
) -> Result<ExportResponse, NanoServiceError>
where
    D: GetUserById,
{
    let user = load_user(dal, user_id).await?;
    publish_event!(ExportRequested {
        user_id: user.id,
        email: user.email.clone(),
    });

    Ok(ExportResponse {
        message: "Export started. Check your email.".to_string(),
    })
}

async fn load_user<D>(dal: &D, user_id: uuid::Uuid) -> Result<kernel::User, NanoServiceError>
where
    D: GetUserById,
{
    dal.get_user_by_id(user_id).await?.ok_or_else(|| {
        NanoServiceError::new("User not found", NanoServiceErrorStatus::Unauthorized)
    })
}

fn validate_new_password(password: &str) -> Result<(), NanoServiceError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(NanoServiceError::new(
            "Password must be at least 8 characters",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    Ok(())
}

async fn ensure_password_matches(
    current_password: &str,
    password_hash: &str,
) -> Result<(), NanoServiceError> {
    if !password::verify_password(current_password, password_hash)? {
        return Err(NanoServiceError::new(
            "Current password is incorrect",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    Ok(())
}

fn export_attachment_filename() -> String {
    format!(
        "drafthouse-export-{}.zip",
        chrono::Utc::now().format("%Y-%m-%d")
    )
}

fn sanitize_export_filename(title: &str) -> String {
    let mut normalized = String::with_capacity(title.len());
    for ch in title.trim().chars() {
        let keep = ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_');
        normalized.push(if keep { ch } else { '-' });
    }

    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join("-");

    let mut cleaned = collapsed
        .trim_matches(|c: char| c == '-' || c == '_' || c == '.')
        .to_ascii_lowercase();
    cleaned = cleaned
        .chars()
        .filter(|c| !c.is_ascii_control())
        .collect::<String>();
    if cleaned.is_empty() {
        return "untitled.md".to_string();
    }

    let stem = cleaned
        .chars()
        .take(MAX_EXPORT_FILENAME_STEM_LEN)
        .collect::<String>()
        .trim_matches(|c: char| c == '-' || c == '_' || c == '.')
        .to_string();

    if stem.is_empty() {
        "untitled.md".to_string()
    } else {
        format!("{stem}.md")
    }
}

fn dedupe_export_filename(base_name: String, seen: &mut HashSet<String>) -> String {
    if seen.insert(base_name.clone()) {
        return base_name;
    }

    let stem = base_name.strip_suffix(".md").unwrap_or(&base_name);
    let mut index = 2;
    loop {
        let candidate = format!("{stem}-{index}.md");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn build_export_zip(entries: &[(String, String)]) -> Result<Vec<u8>, NanoServiceError> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buffer);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (filename, content) in entries {
        zip.start_file(filename, options).map_err(|e| {
            NanoServiceError::new(
                format!("Failed to start export zip file: {e}"),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;
        zip.write_all(content.as_bytes()).map_err(|e| {
            NanoServiceError::new(
                format!("Failed to write export zip file: {e}"),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;
    }

    zip.finish().map_err(|e| {
        NanoServiceError::new(
            format!("Failed to finalize export zip: {e}"),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    Ok(buffer.into_inner())
}

async fn build_export_entries(
    dal: &SqlxPostGresDescriptor,
    user_id: uuid::Uuid,
) -> Result<Vec<(String, String)>, NanoServiceError> {
    let documents = dal.list_documents_by_owner_no_pagination(user_id).await?;
    let mut seen_filenames = HashSet::new();
    let mut entries = Vec::with_capacity(documents.len());

    for document in documents {
        entries.push(build_export_entry(dal, document, &mut seen_filenames).await?);
    }

    Ok(entries)
}

async fn build_export_entry(
    dal: &SqlxPostGresDescriptor,
    document: Document,
    seen_filenames: &mut HashSet<String>,
) -> Result<(String, String), NanoServiceError> {
    let filename =
        dedupe_export_filename(sanitize_export_filename(&document.title), seen_filenames);
    let content = dal
        .get_document_content(document.id)
        .await?
        .unwrap_or_default();

    Ok((filename, content))
}

#[subscribe_to_event]
async fn on_export_requested(event: ExportRequested) {
    if let Err(error) = handle_export_requested(event.clone()).await {
        tracing::error!(
            user_id = %event.user_id,
            email = %event.email,
            error = %error,
            "failed to process gdpr export"
        );
    }
}

async fn handle_export_requested(event: ExportRequested) -> Result<(), NanoServiceError> {
    let dal = EXPORT_DALS
        .lock()
        .expect("export dal mutex poisoned")
        .remove(&event.user_id)
        .ok_or_else(|| {
            NanoServiceError::new(
                "Export DAL not initialized",
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;

    let entries = build_export_entries(&dal, event.user_id).await?;
    let zip_bytes = build_export_zip(&entries)?;
    let attachment = email::EmailAttachment {
        filename: export_attachment_filename(),
        content_base64: STANDARD.encode(zip_bytes),
    };

    email::send_export_email(&event.email, &attachment).await?;
    tracing::info!(
        user_id = %event.user_id,
        email = %event.email,
        document_count = entries.len(),
        "gdpr export email sent"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn verified_user() -> kernel::User {
        kernel::User {
            id: Uuid::new_v4(),
            email: "owner@example.com".to_string(),
            password_hash: crate::password::hash_password("current-password").unwrap(),
            email_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            welcome_doc_created: true,
        }
    }

    struct MockDal {
        user: Option<kernel::User>,
        updated_password: Arc<Mutex<Option<(Uuid, String)>>>,
        revoked_sessions: Arc<Mutex<Vec<Uuid>>>,
        deleted_users: Arc<Mutex<Vec<Uuid>>>,
    }

    impl MockDal {
        fn with_user(user: kernel::User) -> Self {
            Self {
                user: Some(user),
                updated_password: Arc::new(Mutex::new(None)),
                revoked_sessions: Arc::new(Mutex::new(Vec::new())),
                deleted_users: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl GetUserById for MockDal {
        fn get_user_by_id(
            &self,
            _id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<kernel::User>, NanoServiceError>> + Send
        {
            let user = self.user.clone();
            async move { Ok(user) }
        }
    }

    impl dal::UpdateUserPassword for MockDal {
        fn update_user_password(
            &self,
            user_id: Uuid,
            password_hash: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let updated_password = Arc::clone(&self.updated_password);
            async move {
                *updated_password.lock().unwrap() = Some((user_id, password_hash));
                Ok(())
            }
        }
    }

    impl DeleteAllRefreshTokensForUser for MockDal {
        fn delete_all_refresh_tokens_for_user(
            &self,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let revoked_sessions = Arc::clone(&self.revoked_sessions);
            async move {
                revoked_sessions.lock().unwrap().push(user_id);
                Ok(())
            }
        }
    }

    impl DeleteUser for MockDal {
        fn delete_user(
            &self,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let deleted_users = Arc::clone(&self.deleted_users);
            async move {
                deleted_users.lock().unwrap().push(user_id);
                Ok(())
            }
        }
    }

    #[test]
    fn sanitize_filename_removes_reserved_chars_and_falls_back() {
        assert_eq!(
            sanitize_export_filename(" Hello/World:* "),
            "hello-world.md"
        );
        assert_eq!(sanitize_export_filename(""), "untitled.md");
        assert_eq!(sanitize_export_filename("..."), "untitled.md");
    }

    #[test]
    fn sanitize_filename_truncates_long_titles() {
        let title = "A".repeat(200);
        let filename = sanitize_export_filename(&title);
        assert!(filename.ends_with(".md"));
        assert!(filename.len() <= MAX_EXPORT_FILENAME_STEM_LEN + 3);
    }

    #[test]
    fn dedupe_filename_appends_numeric_suffixes() {
        let mut seen = HashSet::new();
        assert_eq!(
            dedupe_export_filename("doc.md".to_string(), &mut seen),
            "doc.md"
        );
        assert_eq!(
            dedupe_export_filename("doc.md".to_string(), &mut seen),
            "doc-2.md"
        );
        assert_eq!(
            dedupe_export_filename("doc.md".to_string(), &mut seen),
            "doc-3.md"
        );
    }

    #[test]
    fn build_export_zip_supports_empty_archives() {
        let zip = build_export_zip(&[]).expect("zip should build");
        assert!(!zip.is_empty());
    }

    #[tokio::test]
    async fn change_password_succeeds_and_revokes_sessions() {
        let user = verified_user();
        let dal = MockDal::with_user(user.clone());

        let response = change_password(&dal, user.id, "current-password", "new-password-123")
            .await
            .expect("password change should succeed");

        assert_eq!(response.message, "Password updated successfully.");
        let updated_password = dal.updated_password.lock().unwrap().clone();
        assert!(updated_password.is_some());
        let revoked = dal.revoked_sessions.lock().unwrap().clone();
        assert_eq!(revoked, vec![user.id]);
    }

    #[tokio::test]
    async fn change_password_rejects_wrong_current_password() {
        let user = verified_user();
        let dal = MockDal::with_user(user.clone());

        let error = change_password(&dal, user.id, "wrong-password", "new-password-123")
            .await
            .expect_err("wrong password should fail");

        assert_eq!(error.status, NanoServiceErrorStatus::BadRequest);
        assert_eq!(&error.message, "Current password is incorrect");
    }

    #[tokio::test]
    async fn delete_account_succeeds_after_password_check() {
        let user = verified_user();
        let dal = MockDal::with_user(user.clone());

        let response = delete_account(&dal, user.id, "current-password")
            .await
            .expect("delete account should succeed");

        assert_eq!(response.message, "Account deleted successfully.");
        assert_eq!(dal.revoked_sessions.lock().unwrap().clone(), vec![user.id]);
        assert_eq!(dal.deleted_users.lock().unwrap().clone(), vec![user.id]);
    }

    #[tokio::test]
    async fn delete_account_rejects_wrong_password() {
        let user = verified_user();
        let dal = MockDal::with_user(user.clone());

        let error = delete_account(&dal, user.id, "wrong-password")
            .await
            .expect_err("wrong password should fail");

        assert_eq!(error.status, NanoServiceErrorStatus::BadRequest);
        assert_eq!(&error.message, "Current password is incorrect");
    }

    #[tokio::test]
    async fn request_export_returns_accepted_message() {
        let user = verified_user();
        let dal = MockDal::with_user(user.clone());

        let response = request_export(&dal, user.id)
            .await
            .expect("export request should succeed");

        assert_eq!(response.message, "Export started. Check your email.");
    }
}
