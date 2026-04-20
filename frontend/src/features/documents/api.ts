import { useAuthStore } from "#/features/auth/store";

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

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
