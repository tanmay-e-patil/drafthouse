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
  const { prependDocument } = useDocumentStore();

  useEffect(() => {
    async function handleKeyDown(e: KeyboardEvent) {
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

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigate, prependDocument]);

  if (!accessToken) {
    return (
      <main className="page-wrap px-4 pb-8 pt-14">
        <section className="island-shell rise-in relative overflow-hidden rounded-[2rem] px-6 py-10 sm:px-10 sm:py-14">
          <p className="island-kicker mb-3">Drafthouse</p>
          <h1 className="display-title mb-5 max-w-3xl text-4xl leading-[1.02] font-bold tracking-tight text-[var(--sea-ink)] sm:text-6xl">
            Collaborative Markdown Editor
          </h1>
          <p className="mb-8 max-w-2xl text-base text-[var(--sea-ink-soft)] sm:text-lg">
            Sign in to create and edit documents in real-time.
          </p>
        </section>
      </main>
    );
  }

  return (
    <div className="dashboard-layout">
      <Sidebar />
      <main className="editor-area">
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flex: 1,
            color: "var(--sea-ink-soft)",
          }}
        >
          <p>Select a document or create a new one</p>
        </div>
      </main>
    </div>
  );
}
