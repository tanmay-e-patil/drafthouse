use dal::{
    CountDocumentsByOwner, CreateDocument, DeleteDocument, GetDocumentById, GetDocumentContent,
    ListDocumentsByOwner, UpdateDocument, UpdateDocumentContent,
};
use kernel::{
    Document, DocumentContentResponse, DocumentListResponse, NewDocument,
    UpdateDocumentContentRequest, UpdateDocumentRequest,
};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

const DEFAULT_PAGE_LIMIT: i64 = 20;

pub async fn create_document<D>(
    dal: &D,
    owner_id: uuid::Uuid,
    title: &str,
) -> Result<Document, NanoServiceError>
where
    D: CreateDocument,
{
    let trimmed = title.trim().to_string();
    let title = if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed
    };

    let doc = dal.create_document(NewDocument { owner_id, title }).await?;
    Ok(doc)
}

pub async fn get_document<D>(dal: &D, id: uuid::Uuid) -> Result<Document, NanoServiceError>
where
    D: GetDocumentById,
{
    dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })
}

pub async fn update_document<D>(
    dal: &D,
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
    request: &UpdateDocumentRequest,
) -> Result<Document, NanoServiceError>
where
    D: GetDocumentById + UpdateDocument,
{
    let existing = dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if existing.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can update it",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    let title = request.title.as_ref().map(|t| t.trim().to_string());
    dal.update_document(id, title, request.is_public).await
}

pub async fn delete_document<D>(
    dal: &D,
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
) -> Result<(), NanoServiceError>
where
    D: GetDocumentById + DeleteDocument,
{
    let existing = dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if existing.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can delete it",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    dal.delete_document(id).await
}

pub async fn list_documents<D>(
    dal: &D,
    owner_id: uuid::Uuid,
    cursor: Option<uuid::Uuid>,
    limit: Option<i64>,
) -> Result<DocumentListResponse, NanoServiceError>
where
    D: ListDocumentsByOwner + CountDocumentsByOwner,
{
    let effective_limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT).min(100);
    let docs = dal
        .list_documents_by_owner(owner_id, cursor, effective_limit + 1)
        .await?;

    let has_more = docs.len() as i64 > effective_limit;
    let data: Vec<Document> = if has_more {
        docs.into_iter().take(effective_limit as usize).collect()
    } else {
        docs
    };

    let next_cursor = if has_more {
        data.last().map(|d| d.id)
    } else {
        None
    };

    Ok(DocumentListResponse {
        data,
        next_cursor,
        has_more,
    })
}

pub async fn get_document_content<D>(
    dal: &D,
    id: uuid::Uuid,
) -> Result<DocumentContentResponse, NanoServiceError>
where
    D: GetDocumentById + GetDocumentContent,
{
    dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    let content = dal.get_document_content(id).await?.unwrap_or_default();
    Ok(DocumentContentResponse { content })
}

pub async fn update_document_content<D>(
    dal: &D,
    id: uuid::Uuid,
    request: &UpdateDocumentContentRequest,
) -> Result<(), NanoServiceError>
where
    D: GetDocumentById + UpdateDocumentContent,
{
    dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    dal.update_document_content(id, request.content.clone())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn test_document(owner_id: Uuid) -> Document {
        Document {
            id: Uuid::new_v4(),
            owner_id,
            title: "Test Document".to_string(),
            is_public: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    struct MockDal {
        documents: Arc<Mutex<Vec<Document>>>,
        next_id: Arc<Mutex<Uuid>>,
        content: Arc<Mutex<std::collections::HashMap<Uuid, String>>>,
    }

    impl MockDal {
        fn new() -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
            }
        }

        fn with_document(doc: Document) -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![doc])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
            }
        }

        fn with_documents(docs: Vec<Document>) -> Self {
            Self {
                documents: Arc::new(Mutex::new(docs)),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
            }
        }

        fn with_document_and_content(doc: Document, content: &str) -> Self {
            let mut map = std::collections::HashMap::new();
            map.insert(doc.id, content.to_string());
            Self {
                documents: Arc::new(Mutex::new(vec![doc])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(map)),
            }
        }
    }

    impl CreateDocument for MockDal {
        fn create_document(
            &self,
            new_doc: NewDocument,
        ) -> impl std::future::Future<Output = Result<Document, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            let id = *self.next_id.lock().unwrap();
            async move {
                let doc = Document {
                    id,
                    owner_id: new_doc.owner_id,
                    title: new_doc.title,
                    is_public: false,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                docs.lock().unwrap().push(doc.clone());
                Ok(doc)
            }
        }
    }

    impl GetDocumentById for MockDal {
        fn get_document_by_id(
            &self,
            id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<Document>, NanoServiceError>> + Send
        {
            let docs = Arc::clone(&self.documents);
            async move { Ok(docs.lock().unwrap().iter().find(|d| d.id == id).cloned()) }
        }
    }

    impl UpdateDocument for MockDal {
        fn update_document(
            &self,
            id: Uuid,
            title: Option<String>,
            is_public: Option<bool>,
        ) -> impl std::future::Future<Output = Result<Document, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            async move {
                let mut docs = docs.lock().unwrap();
                if let Some(doc) = docs.iter_mut().find(|d| d.id == id) {
                    if let Some(t) = title {
                        doc.title = t;
                    }
                    if let Some(p) = is_public {
                        doc.is_public = p;
                    }
                    doc.updated_at = Utc::now();
                    return Ok(doc.clone());
                }
                Err(NanoServiceError::new(
                    "Document not found",
                    NanoServiceErrorStatus::NotFound,
                ))
            }
        }
    }

    impl DeleteDocument for MockDal {
        fn delete_document(
            &self,
            id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            async move {
                docs.lock().unwrap().retain(|d| d.id != id);
                Ok(())
            }
        }
    }

    impl ListDocumentsByOwner for MockDal {
        fn list_documents_by_owner(
            &self,
            owner_id: Uuid,
            cursor: Option<Uuid>,
            limit: i64,
        ) -> impl std::future::Future<Output = Result<Vec<Document>, NanoServiceError>> + Send
        {
            let docs = Arc::clone(&self.documents);
            async move {
                let all = docs.lock().unwrap();
                let owned: Vec<Document> = all
                    .iter()
                    .filter(|d| d.owner_id == owner_id)
                    .cloned()
                    .collect();

                let start_idx = if let Some(cursor_id) = cursor {
                    owned
                        .iter()
                        .position(|d| d.id == cursor_id)
                        .map(|i| i + 1)
                        .unwrap_or(owned.len())
                } else {
                    0
                };

                Ok(owned
                    .into_iter()
                    .skip(start_idx)
                    .take(limit as usize)
                    .collect())
            }
        }
    }

    impl CountDocumentsByOwner for MockDal {
        fn count_documents_by_owner(
            &self,
            owner_id: Uuid,
        ) -> impl std::future::Future<Output = Result<i64, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            async move {
                Ok(docs
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|d| d.owner_id == owner_id)
                    .count() as i64)
            }
        }
    }

    impl GetDocumentContent for MockDal {
        fn get_document_content(
            &self,
            id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<String>, NanoServiceError>> + Send
        {
            let content = Arc::clone(&self.content);
            async move { Ok(content.lock().unwrap().get(&id).cloned()) }
        }
    }

    impl UpdateDocumentContent for MockDal {
        fn update_document_content(
            &self,
            id: Uuid,
            content: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let content_map = Arc::clone(&self.content);
            async move {
                content_map.lock().unwrap().insert(id, content);
                Ok(())
            }
        }
    }

    // ── create_document ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_document_stores_and_returns() {
        let dal = MockDal::new();
        let owner_id = Uuid::new_v4();
        let result = create_document(&dal, owner_id, "My Doc").await;
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc.title, "My Doc");
        assert_eq!(doc.owner_id, owner_id);
        assert_eq!(dal.documents.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn create_document_trims_title() {
        let dal = MockDal::new();
        let result = create_document(&dal, Uuid::new_v4(), "  Spaced  ").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, "Spaced");
    }

    #[tokio::test]
    async fn create_document_empty_title_defaults_to_untitled() {
        let dal = MockDal::new();
        let result = create_document(&dal, Uuid::new_v4(), "  ").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, "Untitled");
    }

    // ── get_document ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_document_returns_existing() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = get_document(&dal, doc.id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, doc.id);
    }

    #[tokio::test]
    async fn get_document_not_found_returns_404() {
        let dal = MockDal::new();
        let result = get_document(&dal, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── update_document ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_document_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = update_document(
            &dal,
            doc.id,
            owner_id,
            &UpdateDocumentRequest {
                title: Some("New Title".to_string()),
                is_public: None,
            },
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, "New Title");
    }

    #[tokio::test]
    async fn update_document_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = update_document(
            &dal,
            doc.id,
            Uuid::new_v4(),
            &UpdateDocumentRequest {
                title: Some("Hacked".to_string()),
                is_public: None,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    #[tokio::test]
    async fn update_document_not_found_returns_404() {
        let dal = MockDal::new();
        let result = update_document(
            &dal,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &UpdateDocumentRequest {
                title: Some("X".to_string()),
                is_public: None,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── delete_document ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_document_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = delete_document(&dal, doc.id, owner_id).await;
        assert!(result.is_ok());
        assert!(dal.documents.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_document_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = delete_document(&dal, doc.id, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    #[tokio::test]
    async fn delete_document_not_found_returns_404() {
        let dal = MockDal::new();
        let result = delete_document(&dal, Uuid::new_v4(), Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── list_documents ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_documents_empty_returns_empty_list() {
        let dal = MockDal::new();
        let result = list_documents(&dal, Uuid::new_v4(), None, None).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(resp.data.is_empty());
        assert!(!resp.has_more);
        assert!(resp.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_documents_returns_owned_docs() {
        let owner_id = Uuid::new_v4();
        let docs = vec![
            test_document(owner_id),
            test_document(owner_id),
            test_document(Uuid::new_v4()),
        ];
        let dal = MockDal::with_documents(docs);
        let result = list_documents(&dal, owner_id, None, None).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.data.len(), 2);
    }

    #[tokio::test]
    async fn list_documents_pagination_with_cursor() {
        let owner_id = Uuid::new_v4();
        let docs: Vec<Document> = (0..5).map(|_| test_document(owner_id)).collect();
        let dal = MockDal::with_documents(docs);

        let first_page = list_documents(&dal, owner_id, None, Some(3)).await.unwrap();
        assert_eq!(first_page.data.len(), 3);
        assert!(first_page.has_more);
        assert!(first_page.next_cursor.is_some());

        let second_page = list_documents(&dal, owner_id, first_page.next_cursor, Some(3))
            .await
            .unwrap();
        assert_eq!(second_page.data.len(), 2);
        assert!(!second_page.has_more);
    }

    // ── get_document_content ──────────────────────────────────────────────────

    #[tokio::test]
    async fn get_document_content_returns_content() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document_and_content(doc.clone(), "# Hello");
        let result = get_document_content(&dal, doc.id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "# Hello");
    }

    #[tokio::test]
    async fn get_document_content_returns_empty_when_no_content() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = get_document_content(&dal, doc.id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "");
    }

    #[tokio::test]
    async fn get_document_content_not_found_returns_404() {
        let dal = MockDal::new();
        let result = get_document_content(&dal, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── update_document_content ───────────────────────────────────────────────

    #[tokio::test]
    async fn update_document_content_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = update_document_content(
            &dal,
            doc.id,
            &UpdateDocumentContentRequest {
                content: "# New Content".to_string(),
            },
        )
        .await;
        assert!(result.is_ok());
        let content = dal.content.lock().unwrap().get(&doc.id).cloned();
        assert_eq!(content, Some("# New Content".to_string()));
    }

    #[tokio::test]
    async fn update_document_content_not_found_returns_404() {
        let dal = MockDal::new();
        let result = update_document_content(
            &dal,
            Uuid::new_v4(),
            &UpdateDocumentContentRequest {
                content: "content".to_string(),
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }
}
