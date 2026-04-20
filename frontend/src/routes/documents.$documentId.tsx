import { createFileRoute, useNavigate, useParams } from "@tanstack/react-router";
import { useEffect, useState, useCallback, useRef } from "react";
import {
  getDocumentApi,
  updateDocumentApi,
  createDocumentApi,
  getDocumentContentApi,
  updateDocumentContentApi,
} from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import Sidebar from "#/components/Sidebar";
import Editor from "#/widgets/editor/Editor";
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
  const [content, setContent] = useState("");
  const [contentLoading, setContentLoading] = useState(true);
  const titleRef = useRef<HTMLInputElement>(null);
  const { prependDocument } = useDocumentStore();

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  const fetchDocument = useCallback(async () => {
    setLoading(true);
    setContentLoading(true);
    try {
      const [doc, contentResp] = await Promise.all([
        getDocumentApi(documentId),
        getDocumentContentApi(documentId),
      ]);
      setDocument(doc);
      setTitle(doc.title);
      setContent(contentResp.content);
    } catch {
      navigate({ to: "/" });
    } finally {
      setLoading(false);
      setContentLoading(false);
    }
  }, [documentId, navigate]);

  useEffect(() => {
    if (hydrated && accessToken) {
      fetchDocument();
    }
  }, [hydrated, accessToken, fetchDocument]);

  useEffect(() => {
    if (!loading && !contentLoading && titleRef.current) {
      titleRef.current.focus();
    }
  }, [loading, contentLoading]);

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

  async function handleContentSave(newContent: string) {
    await updateDocumentContentApi(documentId, newContent);
  }

  async function handleKeyDown(e: Event) {
    const ke = e as KeyboardEvent;
    if ((ke.metaKey || ke.ctrlKey) && ke.key === "n") {
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
    const handler = (e: Event) => handleKeyDown(e as KeyboardEvent);
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (loading) {
    return (
      <div className="editor-layout">
        <Sidebar />
        <main className="editor-area">
          <p style={{ padding: "2rem", color: "var(--ink-soft)" }}>Loading...</p>
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
        {contentLoading ? (
          <div className="editor-content">
            <p style={{ color: "var(--ink-soft)" }}>Loading editor...</p>
          </div>
        ) : (
          <Editor
            key={documentId}
            initialContent={content}
            onSave={handleContentSave}
          />
        )}
      </main>
    </div>
  );
}
