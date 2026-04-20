import { createFileRoute, useNavigate, useParams } from "@tanstack/react-router";
import { useEffect, useState, useCallback, useRef } from "react";
import {
  getDocumentApi,
  updateDocumentApi,
  createDocumentApi,
} from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import Sidebar from "#/components/Sidebar";
import type { Document } from "#/features/documents/api";

export const Route = createFileRoute("/documents/$documentId")({
  component: DocumentEditor,
});

function DocumentEditor() {
  const { documentId } = useParams({ strict: false }) as { documentId: string };
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const [document, setDocument] = useState<Document | null>(null);
  const [title, setTitle] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);
  const { prependDocument } = useDocumentStore();

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  const fetchDocument = useCallback(async () => {
    setLoading(true);
    try {
      const doc = await getDocumentApi(documentId);
      setDocument(doc);
      setTitle(doc.title);
    } catch {
      navigate({ to: "/" });
    } finally {
      setLoading(false);
    }
  }, [documentId, navigate]);

  useEffect(() => {
    if (hydrated && accessToken) {
      fetchDocument();
    }
  }, [hydrated, accessToken, fetchDocument]);

  useEffect(() => {
    if (!loading && titleRef.current) {
      titleRef.current.focus();
    }
  }, [loading]);

  async function handleTitleBlur() {
    if (!document || saving) return;
    const trimmed = title.trim();
    if (trimmed === document.title) return;
    setSaving(true);
    try {
      const updated = await updateDocumentApi(document.id, { title: trimmed || "Untitled" });
      setDocument(updated);
      setTitle(updated.title);
      useDocumentStore.getState().updateDocumentInList(document.id, { title: updated.title });
    } catch {
      setTitle(document.title);
    } finally {
      setSaving(false);
    }
  }

  async function handleKeyDown(e: React.KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "n") {
      e.preventDefault();
      try {
        const doc = await createDocumentApi();
        prependDocument(doc);
        navigate({ to: "/documents/$documentId", params: { documentId: doc.id } });
      } catch {
        // silently fail
      }
    }
  }

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (loading) {
    return (
      <div className="editor-layout">
        <Sidebar />
        <main className="editor-area">
          <p style={{ padding: "2rem", color: "var(--sea-ink-soft)" }}>Loading...</p>
        </main>
      </div>
    );
  }

  if (!document) return null;

  return (
    <div className="editor-layout">
      <Sidebar />
      <main className="editor-area">
        <input
          ref={titleRef}
          className="editor-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={handleTitleBlur}
          disabled={saving}
          placeholder="Untitled"
        />
        <div className="editor-content">
          <p style={{ color: "var(--sea-ink-soft)" }}>
            Start writing here...
          </p>
        </div>
      </main>
    </div>
  );
}
