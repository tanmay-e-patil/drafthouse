import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState, useCallback } from "react";
import { createDocumentApi } from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import Sidebar from "#/components/Sidebar";
import { Button } from "#/components/ui/button";
import { CommandPalette } from "#/features/documents/CommandPalette";
import { useDocumentHotkeys } from "#/features/documents/useDocumentHotkeys";
import { FileText, Plus } from "lucide-react";
import { Link } from "@tanstack/react-router";

export const Route = createFileRoute("/")({ component: Dashboard });

function Dashboard() {
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const { prependDocument } = useDocumentStore();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  const toggleSidebar = useCallback(() => setSidebarCollapsed((v) => !v), []);
  const openPalette = useCallback(() => setPaletteOpen(true), []);

  useDocumentHotkeys({
    onOpenPalette: openPalette,
    onToggleSidebar: toggleSidebar,
  });

  if (!hydrated) return null;

  if (!accessToken) {
    return (
      <main className="flex h-screen flex-col items-center justify-center gap-6">
        <h1 className="text-2xl font-bold tracking-tight">Drafthouse</h1>
        <p className="text-sm text-muted-foreground">
          Collaborative Markdown Editor
        </p>
        <div className="flex gap-3">
          <Button variant="outline" size="sm" render={<Link to="/login" />}>
            Sign in
          </Button>
          <Button size="sm" render={<Link to="/register" />}>
            Sign up
          </Button>
        </div>
      </main>
    );
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
      <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
      <main className="flex flex-1 flex-col overflow-hidden">
        <div className="flex h-12 items-center justify-between border-b border-border px-4">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <FileText className="size-4" />
            <span>Documents</span>
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="gap-1.5 text-muted-foreground"
            onClick={() => {
              createDocumentApi().then((doc) => {
                prependDocument(doc);
                navigate({
                  to: "/documents/$documentId",
                  params: { documentId: doc.id },
                });
              }).catch(() => {});
            }}
          >
            <Plus className="size-3.5" />
            New
          </Button>
        </div>
        <div className="flex flex-1 items-center justify-center p-8">
          <div className="text-center text-muted-foreground">
            <FileText className="mx-auto mb-3 size-10 opacity-30" />
            <p className="text-sm">Select a document or create a new one</p>
            <p className="mt-1 text-xs text-muted-foreground/60">
              Press <kbd className="rounded border border-border bg-muted px-1 py-0.5 text-[10px] font-mono">⌘ K</kbd> to search your documents
            </p>
          </div>
        </div>
      </main>
    </div>
  );
}
