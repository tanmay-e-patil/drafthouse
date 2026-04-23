use kernel::{
    Document, DocumentMember, InviteLink, MemberRole, NewDocument, NewInviteLink, NewWsTicket,
    WsTicket,
};

crate::define_dal_transactions!(
    CreateDocument => create_document(new_doc: NewDocument) -> Document,
    GetDocumentById => get_document_by_id(id: uuid::Uuid) -> Option<Document>,
    UpdateDocument => update_document(id: uuid::Uuid, title: Option<String>, is_public: Option<bool>) -> Document,
    DeleteDocument => delete_document(id: uuid::Uuid) -> (),
    ListDocumentsByOwner => list_documents_by_owner(owner_id: uuid::Uuid, cursor: Option<uuid::Uuid>, limit: i64) -> Vec<Document>,
    ListDocumentsByOwnerNoPagination => list_documents_by_owner_no_pagination(owner_id: uuid::Uuid) -> Vec<Document>,
    CountDocumentsByOwner => count_documents_by_owner(owner_id: uuid::Uuid) -> i64,
    GetDocumentContent => get_document_content(id: uuid::Uuid) -> Option<String>,
    UpdateDocumentContent => update_document_content(id: uuid::Uuid, content: String) -> (),
    CreateWsTicket => create_ws_ticket(new_ticket: NewWsTicket) -> WsTicket,
    GetWsTicketByHash => get_ws_ticket_by_hash(token_hash: String) -> Option<WsTicket>,
    DeleteWsTicket => delete_ws_ticket(token_hash: String) -> (),
    CreateInviteLink => create_invite_link(new_link: NewInviteLink) -> InviteLink,
    GetInviteLinkByToken => get_invite_link_by_token(token: String) -> Option<InviteLink>,
    ListActiveInviteLinks => list_active_invite_links(doc_id: uuid::Uuid) -> Vec<InviteLink>,
    RevokeInviteLink => revoke_invite_link(token: String) -> (),
    AcceptInviteLink => accept_invite_link(token: String, user_id: uuid::Uuid) -> DocumentMember,
    ListDocumentMembers => list_document_members(doc_id: uuid::Uuid) -> Vec<DocumentMember>,
    GetDocumentMember => get_document_member(doc_id: uuid::Uuid, user_id: uuid::Uuid) -> Option<DocumentMember>,
    DeleteDocumentMember => delete_document_member(doc_id: uuid::Uuid, user_id: uuid::Uuid) -> (),
    UpdateDocumentMemberRole => update_document_member_role(doc_id: uuid::Uuid, user_id: uuid::Uuid, role: MemberRole) -> DocumentMember
);
