import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import {
  createDocumentApi,
  listDocumentsApi,
  getDocumentApi,
  updateDocumentApi,
  deleteDocumentApi,
  getDocumentContentApi,
  updateDocumentContentApi,
  createInviteLinkApi,
  listInviteLinksApi,
  revokeInviteLinkApi,
  acceptInviteApi,
  listMembersApi,
  removeMemberApi,
  updateMemberRoleApi,
} from "../api";
import { useAuthStore } from "#/features/auth/store";

const mockDoc = {
  id: "123e4567-e89b-12d3-a456-426614174000",
  owner_id: "123e4567-e89b-12d3-a456-426614174000",
  title: "Test Document",
  is_public: false,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

beforeEach(() => {
  useAuthStore.getState().setAccessToken("test_token");
});

afterEach(() => {
  vi.restoreAllMocks();
  useAuthStore.getState().clearAuth();
});

describe("createDocumentApi", () => {
  it("returns document on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => mockDoc,
      })
    );

    const result = await createDocumentApi("Test Document");
    expect(result.id).toBe(mockDoc.id);
    expect(result.title).toBe("Test Document");
  });

  it("throws with server detail on error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Unauthorized" }),
      })
    );

    await expect(createDocumentApi("Test")).rejects.toThrow("Unauthorized");
  });

  it("sends POST with auth header", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => mockDoc,
    });
    vi.stubGlobal("fetch", mockFetch);

    await createDocumentApi("Test");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer test_token",
          "Content-Type": "application/json",
        }),
        credentials: "include",
      })
    );
  });
});

describe("listDocumentsApi", () => {
  it("returns document list on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          data: [mockDoc],
          next_cursor: null,
          has_more: false,
        }),
      })
    );

    const result = await listDocumentsApi();
    expect(result.data).toHaveLength(1);
    expect(result.has_more).toBe(false);
    expect(result.next_cursor).toBeNull();
  });

  it("passes cursor and limit as query params", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ data: [], next_cursor: null, has_more: false }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await listDocumentsApi("some-cursor", 10);
    const url = mockFetch.mock.calls[0][0];
    expect(url).toContain("cursor=some-cursor");
    expect(url).toContain("limit=10");
  });

  it("throws on error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Server error" }),
      })
    );

    await expect(listDocumentsApi()).rejects.toThrow("Server error");
  });
});

describe("getDocumentApi", () => {
  it("returns single document", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => mockDoc,
      })
    );

    const result = await getDocumentApi(mockDoc.id);
    expect(result.title).toBe("Test Document");
  });

  it("throws with fallback on missing detail", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({}),
      })
    );

    await expect(getDocumentApi("bad-id")).rejects.toThrow(
      "Document not found"
    );
  });
});

describe("updateDocumentApi", () => {
  it("returns updated document", async () => {
    const updated = { ...mockDoc, title: "Updated" };
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => updated,
      })
    );

    const result = await updateDocumentApi(mockDoc.id, { title: "Updated" });
    expect(result.title).toBe("Updated");
  });

  it("sends PATCH with JSON body", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => mockDoc,
    });
    vi.stubGlobal("fetch", mockFetch);

    await updateDocumentApi(mockDoc.id, { title: "New Title" });
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        method: "PATCH",
        body: JSON.stringify({ title: "New Title" }),
      })
    );
  });
});

describe("deleteDocumentApi", () => {
  it("resolves void on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, status: 204 })
    );

    await expect(deleteDocumentApi(mockDoc.id)).resolves.toBeUndefined();
  });

  it("throws on 403 forbidden", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Only the document owner can delete it" }),
      })
    );

    await expect(deleteDocumentApi("bad-id")).rejects.toThrow("owner");
  });
});

describe("getDocumentContentApi", () => {
  it("returns content on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ content: "# Hello" }),
      })
    );

    const result = await getDocumentContentApi(mockDoc.id);
    expect(result.content).toBe("# Hello");
  });

  it("returns empty content for new document", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ content: "" }),
      })
    );

    const result = await getDocumentContentApi(mockDoc.id);
    expect(result.content).toBe("");
  });

  it("throws on 404", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Document not found" }),
      })
    );

    await expect(getDocumentContentApi("bad-id")).rejects.toThrow("Document not found");
  });

  it("sends GET with auth header", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ content: "" }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await getDocumentContentApi(mockDoc.id);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/content"),
      expect.objectContaining({
        method: "GET",
        headers: expect.objectContaining({
          Authorization: "Bearer test_token",
        }),
      })
    );
  });
});

describe("updateDocumentContentApi", () => {
  it("resolves void on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, status: 200 })
    );

    await expect(updateDocumentContentApi(mockDoc.id, "# New content")).resolves.toBeUndefined();
  });

  it("sends PATCH with content body", async () => {
    const mockFetch = vi.fn().mockResolvedValue({ ok: true, status: 200 });
    vi.stubGlobal("fetch", mockFetch);

    await updateDocumentContentApi(mockDoc.id, "# Hello");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/content"),
      expect.objectContaining({
        method: "PATCH",
        body: JSON.stringify({ content: "# Hello" }),
      })
    );
  });

  it("throws on error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Document not found" }),
      })
    );

    await expect(updateDocumentContentApi("bad-id", "content")).rejects.toThrow("Document not found");
  });
});

const mockDocId = "123e4567-e89b-12d3-a456-426614174000";
const mockUserId = "223e4567-e89b-12d3-a456-426614174001";
const mockToken = "abc123def456abc123def456abc12345";

const mockInviteLink = {
  token: mockToken,
  doc_id: mockDocId,
  role: "editor" as const,
  created_by: mockUserId,
  max_uses: null,
  use_count: 0,
  expires_at: null,
  revoked_at: null,
};

const mockMember = {
  doc_id: mockDocId,
  user_id: mockUserId,
  role: "editor" as const,
};

describe("createInviteLinkApi", () => {
  it("returns invite link on success", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => mockInviteLink }));
    const result = await createInviteLinkApi(mockDocId, { role: "editor" });
    expect(result.token).toBe(mockToken);
    expect(result.role).toBe("editor");
  });

  it("sends POST to /documents/:id/invites with role", async () => {
    const mockFetch = vi.fn().mockResolvedValue({ ok: true, json: async () => mockInviteLink });
    vi.stubGlobal("fetch", mockFetch);
    await createInviteLinkApi(mockDocId, { role: "viewer", max_uses: 5 });
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining(`/documents/${mockDocId}/invites`),
      expect.objectContaining({ method: "POST" })
    );
    const body = JSON.parse(mockFetch.mock.calls[0][1].body);
    expect(body.role).toBe("viewer");
    expect(body.max_uses).toBe(5);
  });

  it("throws on 403 forbidden", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({ detail: "Forbidden" }) }));
    await expect(createInviteLinkApi(mockDocId, { role: "editor" })).rejects.toThrow("Forbidden");
  });
});

describe("listInviteLinksApi", () => {
  it("returns array of links", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => [mockInviteLink] }));
    const result = await listInviteLinksApi(mockDocId);
    expect(result).toHaveLength(1);
    expect(result[0].token).toBe(mockToken);
  });

  it("sends GET to /documents/:id/invites", async () => {
    const mockFetch = vi.fn().mockResolvedValue({ ok: true, json: async () => [] });
    vi.stubGlobal("fetch", mockFetch);
    await listInviteLinksApi(mockDocId);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining(`/documents/${mockDocId}/invites`),
      expect.objectContaining({ method: "GET" })
    );
  });
});

describe("revokeInviteLinkApi", () => {
  it("resolves void on 204", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));
    await expect(revokeInviteLinkApi(mockDocId, mockToken)).resolves.toBeUndefined();
  });

  it("throws on error", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({ detail: "Not found" }) }));
    await expect(revokeInviteLinkApi(mockDocId, mockToken)).rejects.toThrow("Not found");
  });
});

describe("acceptInviteApi", () => {
  it("returns member on success", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => mockMember }));
    const result = await acceptInviteApi(mockToken);
    expect(result.role).toBe("editor");
    expect(result.user_id).toBe(mockUserId);
  });

  it("sends POST to /invites/:token/accept", async () => {
    const mockFetch = vi.fn().mockResolvedValue({ ok: true, json: async () => mockMember });
    vi.stubGlobal("fetch", mockFetch);
    await acceptInviteApi(mockToken);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining(`/invites/${mockToken}/accept`),
      expect.objectContaining({ method: "POST" })
    );
  });

  it("throws 410 gone for expired link", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({ detail: "Invite link has expired" }) }));
    await expect(acceptInviteApi(mockToken)).rejects.toThrow("Invite link has expired");
  });
});

describe("listMembersApi", () => {
  it("returns array of members", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => [mockMember] }));
    const result = await listMembersApi(mockDocId);
    expect(result).toHaveLength(1);
    expect(result[0].role).toBe("editor");
  });
});

describe("removeMemberApi", () => {
  it("resolves void on success", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));
    await expect(removeMemberApi(mockDocId, mockUserId)).resolves.toBeUndefined();
  });

  it("throws on error", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({ detail: "Owner cannot remove themselves" }) }));
    await expect(removeMemberApi(mockDocId, mockUserId)).rejects.toThrow("Owner cannot remove themselves");
  });
});

describe("updateMemberRoleApi", () => {
  it("returns updated member", async () => {
    const updated = { ...mockMember, role: "viewer" as const };
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => updated }));
    const result = await updateMemberRoleApi(mockDocId, mockUserId, "viewer");
    expect(result.role).toBe("viewer");
  });

  it("sends PATCH with role body", async () => {
    const mockFetch = vi.fn().mockResolvedValue({ ok: true, json: async () => mockMember });
    vi.stubGlobal("fetch", mockFetch);
    await updateMemberRoleApi(mockDocId, mockUserId, "viewer");
    const body = JSON.parse(mockFetch.mock.calls[0][1].body);
    expect(body.role).toBe("viewer");
  });
});
