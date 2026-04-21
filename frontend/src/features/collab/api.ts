import { useAuthStore } from "#/features/auth/store";

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

export interface WsTicketResponse {
  ticket: string;
}

export async function issueWsTicket(docId: string): Promise<WsTicketResponse> {
  const token = useAuthStore.getState().accessToken;
  const res = await fetch(`${API_BASE}/documents/${docId}/ws-ticket`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    credentials: "include",
  });
  if (!res.ok) throw new Error(`Failed to issue WS ticket: ${res.status}`);
  return res.json();
}
