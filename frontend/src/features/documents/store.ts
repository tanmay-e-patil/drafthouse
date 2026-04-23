import { create } from "zustand";
import type { Document, DocumentPresencePeer } from "./api";

interface DocumentState {
  documents: Document[];
  presenceByDocumentId: Record<string, DocumentPresencePeer[]>;
  nextCursor: string | null;
  hasMore: boolean;
  isLoading: boolean;
  setDocuments: (docs: Document[], nextCursor: string | null, hasMore: boolean) => void;
  setPresenceForDocument: (id: string, peers: DocumentPresencePeer[]) => void;
  replacePresence: (presenceByDocumentId: Record<string, DocumentPresencePeer[]>) => void;
  prependDocument: (doc: Document) => void;
  upsertDocument: (doc: Document) => void;
  updateDocumentInList: (id: string, updates: Partial<Document>) => void;
  removeDocumentFromList: (id: string) => void;
  setLoading: (loading: boolean) => void;
  reset: () => void;
}

export const useDocumentStore = create<DocumentState>((set) => ({
  documents: [],
  presenceByDocumentId: {},
  nextCursor: null,
  hasMore: false,
  isLoading: false,
  setDocuments: (docs, nextCursor, hasMore) =>
    set({ documents: docs, nextCursor, hasMore }),
  setPresenceForDocument: (id, peers) =>
    set((state) => ({
      presenceByDocumentId: {
        ...state.presenceByDocumentId,
        [id]: peers,
      },
    })),
  replacePresence: (presenceByDocumentId) => set({ presenceByDocumentId }),
  prependDocument: (doc) =>
    set((state) => ({
      documents: [doc, ...state.documents],
    })),
  upsertDocument: (doc) =>
    set((state) => {
      const existingIndex = state.documents.findIndex((d) => d.id === doc.id);
      if (existingIndex === -1) {
        return { documents: [doc, ...state.documents] };
      }

      return {
        documents: state.documents.map((existing) =>
          existing.id === doc.id ? doc : existing
        ),
      };
    }),
  updateDocumentInList: (id, updates) =>
    set((state) => ({
      documents: state.documents.map((d) =>
        d.id === id ? { ...d, ...updates } : d
      ),
    })),
  removeDocumentFromList: (id) =>
    set((state) => ({
      documents: state.documents.filter((d) => d.id !== id),
      presenceByDocumentId: Object.fromEntries(
        Object.entries(state.presenceByDocumentId).filter(([docId]) => docId !== id)
      ),
    })),
  setLoading: (loading) => set({ isLoading: loading }),
  reset: () =>
    set({
      documents: [],
      presenceByDocumentId: {},
      nextCursor: null,
      hasMore: false,
      isLoading: false,
    }),
}));
