import { useAuthStore } from "#/features/auth/store";

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

export type MemberRole = "editor" | "viewer";

export interface Document {
  id: string;
  owner_id: string;
  title: string;
  is_public: boolean;
  created_at: string;
  updated_at: string;
}

export interface DocumentListResponse {
  data: Document[];
  next_cursor: string | null;
  has_more: boolean;
}

export interface ApiError {
  detail: string;
}

export interface DocumentContentResponse {
  content: string;
}

function getAuthHeaders(): Record<string, string> {
  const token = useAuthStore.getState().accessToken;
  return {
    "Content-Type": "application/json",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };
}

function handleResponse<T>(res: Response, fallbackError: string): Promise<T> {
  if (!res.ok) {
    return res.json().then((data) => {
      const err = data as ApiError;
      throw new Error(err.detail ?? fallbackError);
    });
  }
  return res.json() as Promise<T>;
}

export async function createDocumentApi(
  title?: string
): Promise<Document> {
  const res = await fetch(`${API_BASE}/documents`, {
    method: "POST",
    headers: getAuthHeaders(),
    body: JSON.stringify({ title: title ?? null }),
    credentials: "include",
  });
  return handleResponse<Document>(res, "Failed to create document");
}

export async function listDocumentsApi(
  cursor?: string | null,
  limit?: number
): Promise<DocumentListResponse> {
  const params = new URLSearchParams();
  if (cursor) params.set("cursor", cursor);
  if (limit) params.set("limit", String(limit));
  const qs = params.toString();
  const url = `${API_BASE}/documents${qs ? `?${qs}` : ""}`;

  const res = await fetch(url, {
    method: "GET",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  return handleResponse<DocumentListResponse>(res, "Failed to list documents");
}

export async function getDocumentApi(id: string): Promise<Document> {
  const res = await fetch(`${API_BASE}/documents/${id}`, {
    method: "GET",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  return handleResponse<Document>(res, "Document not found");
}

export async function updateDocumentApi(
  id: string,
  data: { title?: string; is_public?: boolean }
): Promise<Document> {
  const res = await fetch(`${API_BASE}/documents/${id}`, {
    method: "PATCH",
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
    credentials: "include",
  });
  return handleResponse<Document>(res, "Failed to update document");
}

export async function deleteDocumentApi(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${id}`, {
    method: "DELETE",
    headers: getAuthHeaders(),
    credentials: "include",
  });

  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    const err = data as ApiError;
    throw new Error(err.detail ?? "Failed to delete document");
  }
}

export async function getDocumentContentApi(
  id: string
): Promise<DocumentContentResponse> {
  const res = await fetch(`${API_BASE}/documents/${id}/content`, {
    method: "GET",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  return handleResponse<DocumentContentResponse>(res, "Failed to get document content");
}

export async function updateDocumentContentApi(
  id: string,
  content: string
): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${id}/content`, {
    method: "PATCH",
    headers: getAuthHeaders(),
    body: JSON.stringify({ content }),
    credentials: "include",
  });

  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    const err = data as ApiError;
    throw new Error(err.detail ?? "Failed to save document content");
  }
}

export interface InviteLink {
  token: string;
  doc_id: string;
  role: MemberRole;
  created_by: string;
  max_uses: number | null;
  use_count: number;
  expires_at: string | null;
  revoked_at: string | null;
}

export interface DocumentMember {
  doc_id: string;
  user_id: string;
  role: MemberRole;
}

export interface CreateInviteLinkRequest {
  role: MemberRole;
  expires_at?: string | null;
  max_uses?: number | null;
}

export async function createInviteLinkApi(
  docId: string,
  req: CreateInviteLinkRequest
): Promise<InviteLink> {
  const res = await fetch(`${API_BASE}/documents/${docId}/invites`, {
    method: "POST",
    headers: getAuthHeaders(),
    body: JSON.stringify(req),
    credentials: "include",
  });
  return handleResponse<InviteLink>(res, "Failed to create invite link");
}

export async function listInviteLinksApi(docId: string): Promise<InviteLink[]> {
  const res = await fetch(`${API_BASE}/documents/${docId}/invites`, {
    method: "GET",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  return handleResponse<InviteLink[]>(res, "Failed to list invite links");
}

export async function revokeInviteLinkApi(
  docId: string,
  token: string
): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${docId}/invites/${token}`, {
    method: "DELETE",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error((data as ApiError).detail ?? "Failed to revoke invite link");
  }
}

export async function acceptInviteApi(token: string): Promise<DocumentMember> {
  const res = await fetch(`${API_BASE}/invites/${token}/accept`, {
    method: "POST",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  return handleResponse<DocumentMember>(res, "Failed to accept invite");
}

export async function listMembersApi(docId: string): Promise<DocumentMember[]> {
  const res = await fetch(`${API_BASE}/documents/${docId}/members`, {
    method: "GET",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  return handleResponse<DocumentMember[]>(res, "Failed to list members");
}

export async function removeMemberApi(
  docId: string,
  userId: string
): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${docId}/members/${userId}`, {
    method: "DELETE",
    headers: getAuthHeaders(),
    credentials: "include",
  });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error((data as ApiError).detail ?? "Failed to remove member");
  }
}

export async function updateMemberRoleApi(
  docId: string,
  userId: string,
  role: MemberRole
): Promise<DocumentMember> {
  const res = await fetch(`${API_BASE}/documents/${docId}/members/${userId}`, {
    method: "PATCH",
    headers: getAuthHeaders(),
    body: JSON.stringify({ role }),
    credentials: "include",
  });
  return handleResponse<DocumentMember>(res, "Failed to update member role");
}
