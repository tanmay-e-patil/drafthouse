import { createFileRoute, useParams } from "@tanstack/react-router";
import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import {
  getDocumentApi,
  updateDocumentApi,
  getDocumentContentApi,
  updateDocumentContentApi,
} from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import Sidebar from "#/components/Sidebar";
import Editor from "#/widgets/editor/Editor";
import { ShareModal } from "#/features/documents/ShareModal";
import type { Document } from "#/features/documents/api";
import { Button, buttonVariants } from "#/components/ui/button";
import { CommandPalette } from "#/features/documents/CommandPalette";
import { useDocumentHotkeys } from "#/features/documents/useDocumentHotkeys";
import { isInaccessibleDocumentError, notifyTransientError } from "#/shared/errors";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "#/components/ui/tooltip";
import {
  EDITOR_FONT_OPTIONS,
  usePreferencesStore,
  type EditorFont,
} from "#/features/preferences/store";
import { Maximize2, Minimize2, Share2 } from "lucide-react";
import { Link } from "@tanstack/react-router";

export const Route = createFileRoute("/documents/$documentId")({
  component: DocumentEditor,
});

function getJwtSubject(token: string | null): string | null {
  if (!token) return null;

  try {
    const payload = token.split(".")[1];
    if (!payload) return null;
    const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
    const decoded = atob(normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "="));
    const claims = JSON.parse(decoded) as { sub?: unknown };
    return typeof claims.sub === "string" ? claims.sub : null;
  } catch {
    return null;
  }
}

function DocumentEditor() {
  const { documentId } = useParams({ strict: false }) as {
    documentId: string;
  };
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const [document, setDocument] = useState<Document | null>(null);
  const [title, setTitle] = useState("");
  const [loading, setLoading] = useState(true);
  const [inaccessibleDocument, setInaccessibleDocument] = useState(false);
  const [saving, setSaving] = useState(false);
  const [shareOpen, setShareOpen] = useState(false);
  const [content, setContent] = useState("");
  const [contentLoading, setContentLoading] = useState(true);
  const titleRef = useRef<HTMLInputElement>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const upsertDocument = useDocumentStore((s) => s.upsertDocument);
  const focusMode = usePreferencesStore((s) => s.focusMode);
  const setFocusMode = usePreferencesStore((s) => s.setFocusMode);
  const toggleFocusMode = usePreferencesStore((s) => s.toggleFocusMode);
  const editorFont = usePreferencesStore((s) => s.editorFont);
  const setEditorFont = usePreferencesStore((s) => s.setEditorFont);
  const currentUserId = useMemo(() => getJwtSubject(accessToken), [accessToken]);
  const fontClassName =
    EDITOR_FONT_OPTIONS.find((option) => option.value === editorFont)?.className ??
    "font-sans";

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  const fetchDocument = useCallback(async () => {
    setLoading(true);
    setContentLoading(true);
    try {
      setInaccessibleDocument(false);
      const [doc, contentResp] = await Promise.all([
        getDocumentApi(documentId),
        getDocumentContentApi(documentId),
      ]);
      setDocument(doc);
      setTitle(doc.title);
      setContent(contentResp.content);
      upsertDocument(doc);
    } catch (error) {
      if (isInaccessibleDocumentError(error)) {
        setInaccessibleDocument(true);
      } else {
        notifyTransientError(error);
      }
    } finally {
      setLoading(false);
      setContentLoading(false);
    }
  }, [documentId, upsertDocument]);

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
      const updated = await updateDocumentApi(document.id, {
        title: trimmed || "Untitled",
      });
      setDocument(updated);
      setTitle(updated.title);
      useDocumentStore
        .getState()
        .updateDocumentInList(document.id, { title: updated.title });
    } catch (error) {
      notifyTransientError(error);
      setTitle(document.title);
    } finally {
      setSaving(false);
    }
  }

  async function handleContentSave(newContent: string) {
    try {
      await updateDocumentContentApi(documentId, newContent);
    } catch (error) {
      notifyTransientError(error);
      throw error;
    }
  }

  function handleRemoteTitleUpdate(newTitle: string) {
    setTitle(newTitle);
    if (document) {
      setDocument({ ...document, title: newTitle });
      useDocumentStore
        .getState()
        .updateDocumentInList(document.id, { title: newTitle });
    }
  }

  const toggleSidebar = useCallback(
    () => setSidebarCollapsed((v) => !v),
    [],
  );
  const openPalette = useCallback(() => setPaletteOpen(true), []);

  useDocumentHotkeys({
    onOpenPalette: openPalette,
    onToggleSidebar: toggleSidebar,
    onToggleFocusMode: toggleFocusMode,
  });

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setFocusMode(false);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [setFocusMode]);

  if (loading) {
    return (
      <div className="flex h-screen overflow-hidden bg-background">
        <CommandPalette
          currentDocumentId={documentId}
          open={paletteOpen}
          onOpenChange={setPaletteOpen}
        />
        {!focusMode && (
          <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
        )}
        <main className="flex flex-1 items-center justify-center text-muted-foreground">
          <p className="text-sm">Loading...</p>
        </main>
      </div>
    );
  }

  if (inaccessibleDocument) {
    return (
      <div className="flex h-screen overflow-hidden bg-background">
        <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
        <main className="flex flex-1 items-center justify-center p-8">
          <div className="ambient-panel max-w-sm rounded-3xl border border-border/80 p-8 text-center shadow-lg">
            <h1 className="font-heading text-xl font-semibold tracking-tight">Document unavailable</h1>
            <p className="mt-2 text-sm text-muted-foreground">
              This document was deleted, or you do not have access to it.
            </p>
            <Link className={buttonVariants({ className: "mt-6" })} to="/">
              Back to dashboard
            </Link>
          </div>
        </main>
      </div>
    );
  }

  if (!document) return null;

  const isOwner = currentUserId === document.owner_id;

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <CommandPalette
        currentDocumentId={documentId}
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
      />
      {!focusMode && (
        <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
      )}
      <main className="flex flex-1 flex-col overflow-hidden bg-background/65">
        {!focusMode && (
          <div className="flex h-12 items-center justify-between border-b border-border/80 bg-card/65 px-4 shadow-xs backdrop-blur">
            <input
              ref={titleRef}
              className="min-w-0 border-none bg-transparent font-heading text-sm font-semibold text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:text-primary"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onBlur={handleTitleBlur}
              disabled={saving}
              placeholder="Untitled"
            />
            <div className="flex items-center gap-2">
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                Font
                <select
                  className="h-8 rounded-md border border-input bg-card/80 px-2 text-xs text-foreground shadow-xs transition-all focus:border-ring focus:outline-none focus:ring-2 focus:ring-ring/30"
                  value={editorFont}
                  onChange={(event) => setEditorFont(event.target.value as EditorFont)}
                  aria-label="Editor font"
                >
                  {EDITOR_FONT_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <Button
                variant="ghost"
                size="sm"
                className="gap-1.5 text-muted-foreground hover:text-accent-foreground"
                onClick={toggleFocusMode}
                title="Toggle focus mode (Cmd/Ctrl+Shift+.)"
              >
                <Maximize2 className="size-3.5" />
                Focus
              </Button>
              <Tooltip>
                <TooltipTrigger render={<span />}>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="gap-1.5 text-muted-foreground hover:text-accent-foreground"
                    onClick={() => setShareOpen(true)}
                    disabled={!isOwner}
                  >
                    <Share2 className="size-3.5" />
                    Share
                  </Button>
                </TooltipTrigger>
                {!isOwner && (
                  <TooltipContent>Only owners can share documents</TooltipContent>
                )}
              </Tooltip>
            </div>
          </div>
        )}
        {focusMode && (
          <Button
            variant="secondary"
            size="sm"
            className="absolute right-4 top-4 z-30 gap-1.5 shadow-md shadow-primary/20"
            onClick={toggleFocusMode}
            aria-label="Exit focus mode"
          >
            <Minimize2 className="size-3.5" />
            Exit focus
          </Button>
        )}
        {shareOpen && (
          <ShareModal
            docId={document.id}
            docTitle={title}
            isPublic={document.is_public}
            onClose={() => setShareOpen(false)}
            onPublicToggle={(isPublic) => {
              setDocument({ ...document, is_public: isPublic });
            }}
          />
        )}
        {contentLoading ? (
          <div className="flex flex-1 items-center justify-center text-muted-foreground">
            <p className="text-sm">Loading editor...</p>
          </div>
        ) : (
          <Editor
            key={documentId}
            docId={documentId}
            initialContent={content}
            onSave={handleContentSave}
            onTitleUpdate={handleRemoteTitleUpdate}
            focusMode={focusMode}
            fontClassName={fontClassName}
          />
        )}
      </main>
    </div>
  );
}
