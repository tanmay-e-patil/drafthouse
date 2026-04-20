import { create } from "zustand";
import type { Document } from "./api";

interface DocumentState {
  documents: Document[];
  nextCursor: string | null;
  hasMore: boolean;
  isLoading: boolean;
  setDocuments: (docs: Document[], nextCursor: string | null, hasMore: boolean) => void;
  prependDocument: (doc: Document) => void;
  updateDocumentInList: (id: string, updates: Partial<Document>) => void;
  removeDocumentFromList: (id: string) => void;
  setLoading: (loading: boolean) => void;
  reset: () => void;
}

export const useDocumentStore = create<DocumentState>((set) => ({
  documents: [],
  nextCursor: null,
  hasMore: false,
  isLoading: false,
  setDocuments: (docs, nextCursor, hasMore) =>
    set({ documents: docs, nextCursor, hasMore }),
  prependDocument: (doc) =>
    set((state) => ({
      documents: [doc, ...state.documents],
    })),
  updateDocumentInList: (id, updates) =>
    set((state) => ({
      documents: state.documents.map((d) =>
        d.id === id ? { ...d, ...updates } : d
      ),
    })),
  removeDocumentFromList: (id) =>
    set((state) => ({
      documents: state.documents.filter((d) => d.id !== id),
    })),
  setLoading: (loading) => set({ isLoading: loading }),
  reset: () =>
    set({ documents: [], nextCursor: null, hasMore: false, isLoading: false }),
}));
