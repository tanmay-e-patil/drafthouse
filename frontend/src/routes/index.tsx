import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";
import { createDocumentApi } from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import Sidebar from "#/components/Sidebar";

export const Route = createFileRoute("/")({ component: Dashboard });

function Dashboard() {
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const { prependDocument } = useDocumentStore();

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  useEffect(() => {
    async function handleKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "n") {
        e.preventDefault();
        try {
          const doc = await createDocumentApi();
          prependDocument(doc);
          navigate({ to: "/documents/$documentId", params: { documentId: doc.id } });
        } catch {
        }
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigate, prependDocument]);

  if (!hydrated) {
    return null;
  }

  if (!accessToken) {
    return (
      <main className="landing-page">
        <h1>Drafthouse</h1>
        <p>Collaborative Markdown Editor</p>
        <div className="landing-actions">
          <a href="/login">Sign in</a>
          <a href="/register">Sign up</a>
        </div>
      </main>
    );
  }

  return (
    <div className="dashboard-layout">
      <Sidebar />
      <main className="editor-area">
        <div className="dashboard-placeholder">
          <p>Select a document or create a new one</p>
        </div>
      </main>
    </div>
  );
}
