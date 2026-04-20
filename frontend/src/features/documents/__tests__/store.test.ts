import { describe, it, expect, beforeEach } from "vitest";
import { useDocumentStore } from "../store";
import type { Document } from "../api";

const mockDoc: Document = {
  id: "11111111-1111-1111-1111-111111111111",
  owner_id: "22222222-2222-2222-2222-222222222222",
  title: "Test Doc",
  is_public: false,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

const mockDoc2: Document = {
  id: "33333333-3333-3333-3333-333333333333",
  owner_id: "22222222-2222-2222-2222-222222222222",
  title: "Another Doc",
  is_public: false,
  created_at: "2024-01-02T00:00:00Z",
  updated_at: "2024-01-02T00:00:00Z",
};

beforeEach(() => {
  useDocumentStore.getState().reset();
});

describe("useDocumentStore", () => {
  it("starts with empty state", () => {
    const state = useDocumentStore.getState();
    expect(state.documents).toEqual([]);
    expect(state.nextCursor).toBeNull();
    expect(state.hasMore).toBe(false);
    expect(state.isLoading).toBe(false);
  });

  it("setDocuments updates list and pagination", () => {
    useDocumentStore.getState().setDocuments([mockDoc], "cursor123", true);
    const state = useDocumentStore.getState();
    expect(state.documents).toHaveLength(1);
    expect(state.nextCursor).toBe("cursor123");
    expect(state.hasMore).toBe(true);
  });

  it("prependDocument adds to front of list", () => {
    useDocumentStore.getState().setDocuments([mockDoc], null, false);
    useDocumentStore.getState().prependDocument(mockDoc2);
    const state = useDocumentStore.getState();
    expect(state.documents).toHaveLength(2);
    expect(state.documents[0].id).toBe(mockDoc2.id);
    expect(state.documents[1].id).toBe(mockDoc.id);
  });

  it("updateDocumentInList updates matching document", () => {
    useDocumentStore.getState().setDocuments([mockDoc, mockDoc2], null, false);
    useDocumentStore.getState().updateDocumentInList(mockDoc.id, {
      title: "Updated",
    });
    const state = useDocumentStore.getState();
    expect(state.documents[0].title).toBe("Updated");
    expect(state.documents[1].title).toBe("Another Doc");
  });

  it("removeDocumentFromList removes matching document", () => {
    useDocumentStore.getState().setDocuments([mockDoc, mockDoc2], null, false);
    useDocumentStore.getState().removeDocumentFromList(mockDoc.id);
    const state = useDocumentStore.getState();
    expect(state.documents).toHaveLength(1);
    expect(state.documents[0].id).toBe(mockDoc2.id);
  });

  it("reset clears all state", () => {
    useDocumentStore.getState().setDocuments([mockDoc], "cursor", true);
    useDocumentStore.getState().setLoading(true);
    useDocumentStore.getState().reset();
    const state = useDocumentStore.getState();
    expect(state.documents).toEqual([]);
    expect(state.nextCursor).toBeNull();
    expect(state.hasMore).toBe(false);
    expect(state.isLoading).toBe(false);
  });

  it("setLoading toggles loading state", () => {
    useDocumentStore.getState().setLoading(true);
    expect(useDocumentStore.getState().isLoading).toBe(true);
    useDocumentStore.getState().setLoading(false);
    expect(useDocumentStore.getState().isLoading).toBe(false);
  });
});
