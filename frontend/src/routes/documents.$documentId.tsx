import { createFileRoute, useNavigate, useParams } from "@tanstack/react-router";
import { useEffect, useState, useCallback, useRef } from "react";
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
import { Button } from "#/components/ui/button";
import { CommandPalette } from "#/features/documents/CommandPalette";
import { useDocumentHotkeys } from "#/features/documents/useDocumentHotkeys";
import {
  EDITOR_FONT_OPTIONS,
  usePreferencesStore,
  type EditorFont,
} from "#/features/preferences/store";
import { Maximize2, Minimize2, Share2 } from "lucide-react";

export const Route = createFileRoute("/documents/$documentId")({
  component: DocumentEditor,
});

function DocumentEditor() {
  const { documentId } = useParams({ strict: false }) as {
    documentId: string;
  };
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const [document, setDocument] = useState<Document | null>(null);
  const [title, setTitle] = useState("");
  const [loading, setLoading] = useState(true);
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
      const [doc, contentResp] = await Promise.all([
        getDocumentApi(documentId),
        getDocumentContentApi(documentId),
      ]);
      setDocument(doc);
      setTitle(doc.title);
      setContent(contentResp.content);
      upsertDocument(doc);
    } catch {
      navigate({ to: "/" });
    } finally {
      setLoading(false);
      setContentLoading(false);
    }
  }, [documentId, navigate, upsertDocument]);

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
    } catch {
      setTitle(document.title);
    } finally {
      setSaving(false);
    }
  }

  async function handleContentSave(newContent: string) {
    await updateDocumentContentApi(documentId, newContent);
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
  });

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const key = event.key.toLowerCase();
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && key === "f") {
        event.preventDefault();
        toggleFocusMode();
        return;
      }

      if (event.key === "Escape") {
        setFocusMode(false);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [setFocusMode, toggleFocusMode]);

  if (loading) {
    return (
      <div className="flex h-screen overflow-hidden">
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

  if (!document) return null;

  return (
    <div className="flex h-screen overflow-hidden">
      <CommandPalette
        currentDocumentId={documentId}
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
      />
      {!focusMode && (
        <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
      )}
      <main className="flex flex-1 flex-col overflow-hidden">
        {!focusMode && (
          <div className="flex h-12 items-center justify-between border-b border-border px-4">
            <input
              ref={titleRef}
              className="border-none bg-transparent text-sm font-medium text-foreground outline-none placeholder:text-muted-foreground"
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
                  className="h-8 rounded-md border border-input bg-background px-2 text-xs text-foreground"
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
                className="gap-1.5 text-muted-foreground"
                onClick={toggleFocusMode}
              >
                <Maximize2 className="size-3.5" />
                Focus
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="gap-1.5 text-muted-foreground"
                onClick={() => setShareOpen(true)}
              >
                <Share2 className="size-3.5" />
                Share
              </Button>
            </div>
          </div>
        )}
        {focusMode && (
          <Button
            variant="secondary"
            size="sm"
            className="absolute right-4 top-4 z-30 gap-1.5 shadow-md"
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
