import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import {
  createDocumentApi,
  listDocumentsApi,
  getDocumentApi,
  updateDocumentApi,
  deleteDocumentApi,
  getDocumentContentApi,
  updateDocumentContentApi,
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
