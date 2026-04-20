use kernel::{Document, NewDocument};

crate::define_dal_transactions!(
    CreateDocument => create_document(new_doc: NewDocument) -> Document,
    GetDocumentById => get_document_by_id(id: uuid::Uuid) -> Option<Document>,
    UpdateDocument => update_document(id: uuid::Uuid, title: Option<String>, is_public: Option<bool>) -> Document,
    DeleteDocument => delete_document(id: uuid::Uuid) -> (),
    ListDocumentsByOwner => list_documents_by_owner(owner_id: uuid::Uuid, cursor: Option<uuid::Uuid>, limit: i64) -> Vec<Document>,
    CountDocumentsByOwner => count_documents_by_owner(owner_id: uuid::Uuid) -> i64,
    GetDocumentContent => get_document_content(id: uuid::Uuid) -> Option<String>,
    UpdateDocumentContent => update_document_content(id: uuid::Uuid, content: String) -> ()
);
