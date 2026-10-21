import { API_BASE } from "#/shared/apiConfig";
import { authFetch } from "#/shared/authFetch";

export interface WsTicketResponse {
  ticket: string;
}

export async function issueWsTicket(docId: string): Promise<WsTicketResponse> {
  const res = await authFetch(`${API_BASE}/documents/${docId}/ws-ticket`, {
    method: "POST",
  });
  if (!res.ok) throw new Error(`Failed to issue WS ticket: ${res.status}`);
  return res.json();
}
