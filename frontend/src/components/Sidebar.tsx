import { useCallback, useEffect, useRef } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import {
  listDocumentsApi,
  deleteDocumentApi,
  createDocumentApi,
} from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import type { Document } from "#/features/documents/api";

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

export default function Sidebar() {
  const navigate = useNavigate();
  const params = useParams({ strict: false }) as { documentId?: string };
  const {
    documents,
    hasMore,
    nextCursor,
    isLoading,
    setDocuments,
    prependDocument,
    removeDocumentFromList,
    setLoading,
  } = useDocumentStore();

  const hydrated = useAuthStore((s) => s.hydrated);
  const accessToken = useAuthStore((s) => s.accessToken);
  const hasFetched = useRef(false);

  const fetchDocuments = useCallback(
    async (cursor?: string | null) => {
      setLoading(true);
      try {
        const resp = await listDocumentsApi(cursor);
        if (!cursor) {
          setDocuments(resp.data, resp.next_cursor, resp.has_more);
        } else {
          setDocuments(
            [...documents, ...resp.data],
            resp.next_cursor,
            resp.has_more
          );
        }
      } catch {
        // silently fail - empty list shown
      } finally {
        setLoading(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [setDocuments, setLoading, documents]
  );

  useEffect(() => {
    if (hydrated && accessToken && !hasFetched.current) {
      hasFetched.current = true;
      fetchDocuments();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated, accessToken]);

  async function handleCreate() {
    try {
      const doc = await createDocumentApi();
      prependDocument(doc);
      navigate({ to: "/documents/$documentId", params: { documentId: doc.id } });
    } catch {
      // silently fail
    }
  }

  async function handleDelete(e: React.MouseEvent, doc: Document) {
    e.preventDefault();
    e.stopPropagation();
    if (!confirm(`Delete "${doc.title}"?`)) return;
    try {
      await deleteDocumentApi(doc.id);
      removeDocumentFromList(doc.id);
      if (params.documentId === doc.id) {
        navigate({ to: "/" });
      }
    } catch {
      // silently fail
    }
  }

  function handleLoadMore() {
    if (nextCursor && hasMore && !isLoading) {
      fetchDocuments(nextCursor);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "n") {
      e.preventDefault();
      handleCreate();
    }
  }

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (documents.length === 0 && !isLoading) {
    return (
      <aside className="sidebar">
        <div className="sidebar-header">
          <h2>Documents</h2>
          <button className="new-doc-btn" onClick={handleCreate}>
            + New
          </button>
        </div>
        <div className="empty-state">
          <p>No documents yet</p>
          <button onClick={handleCreate}>Create your first document</button>
        </div>
      </aside>
    );
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <h2>Documents</h2>
        <button className="new-doc-btn" onClick={handleCreate}>
          + New
        </button>
      </div>
      <div className="doc-list">
        {documents.map((doc) => (
          <a
            key={doc.id}
            href={`/documents/${doc.id}`}
            className={`doc-list-item ${params.documentId === doc.id ? "active" : ""}`}
            onClick={(e) => {
              e.preventDefault();
              navigate({
                to: "/documents/$documentId",
                params: { documentId: doc.id },
              });
            }}
          >
            <div className="doc-list-item-info">
              <div className="doc-list-item-title">{doc.title}</div>
              <div className="doc-list-item-time">
                {formatRelativeTime(doc.updated_at)}
              </div>
            </div>
            <button
              className="doc-list-item-delete"
              onClick={(e) => handleDelete(e, doc)}
              title="Delete document"
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14" />
              </svg>
            </button>
          </a>
        ))}
      </div>
      {hasMore && (
        <button
          className="load-more-btn"
          onClick={handleLoadMore}
          disabled={isLoading}
        >
          {isLoading ? "Loading..." : "Load more"}
        </button>
      )}
    </aside>
  );
}
