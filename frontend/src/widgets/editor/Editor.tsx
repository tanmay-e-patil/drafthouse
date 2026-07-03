import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useCollabStore, type ConnectionStatus } from "#/features/collab/store";
import { useCollabEditor } from "#/features/collab/useCollabEditor";
import AvatarStrip from "#/features/collab/ui/AvatarStrip";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import type { ViewUpdate } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { useDebounce } from "./useDebounce";
import { Toggle } from "#/components/ui/toggle";
import { Button } from "#/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "#/components/ui/tooltip";
import { Eye, Code2 } from "lucide-react";
import { EDITOR_ACTIONS } from "./editorActions";
import { getFormattingEdit, type FormattingActionId } from "./formatting";
import { cn } from "#/lib/utils";
import MarkdownIt from "markdown-it";

interface EditorProps {
  docId: string;
  initialContent: string;
  onSave: (content: string) => Promise<void>;
  onTitleUpdate?: (title: string) => void;
  focusMode?: boolean;
  fontClassName?: string;
  readOnly?: boolean;
}

const DEBOUNCE_MS = 500;

function sanitizeMarkdownPreview(markdown: string) {
  return new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true,
  }).render(markdown);
}

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  connecting: "Connecting...",
  connected: "Synced",
  syncing: "Syncing...",
  disconnected: "Offline",
};

const STATUS_DOT: Record<ConnectionStatus, string> = {
  connecting: "bg-primary",
  connected: "bg-emerald-500 dark:bg-emerald-400",
  syncing: "bg-primary",
  disconnected: "bg-destructive",
};

function dispatchEditorAction(view: EditorView, actionId: FormattingActionId) {
  const selection = view.state.selection.main;
  const edit = getFormattingEdit(actionId, view.state.doc.toString(), {
    from: selection.from,
    to: selection.to,
  });

  view.dispatch({
    changes: {
      from: edit.from,
      to: edit.to,
      insert: edit.insert,
    },
    selection: {
      anchor: edit.selection.from,
      head: edit.selection.to,
    },
    scrollIntoView: true,
  });
  view.focus();
  return true;
}

export default function Editor({
  docId,
  initialContent,
  onSave,
  onTitleUpdate,
  focusMode = false,
  fontClassName = "font-sans",
  readOnly = false,
}: EditorProps) {
  const collabStatus = useCollabStore((s) => s.status);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const currentMode = readOnly ? "preview" : mode;
  const [content, setContent] = useState(initialContent);
  const [saving, setSaving] = useState(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);

  const [editorView, setEditorView] = useState<EditorView | null>(null);
  const [selectionToolbar, setSelectionToolbar] = useState<{
    open: boolean;
    left: number;
    top: number;
  }>({ open: false, left: 0, top: 0 });
  const saveLockRef = useRef(false);

  const debouncedSave = useDebounce(async (value: string) => {
    if (saveLockRef.current) return;
    saveLockRef.current = true;
    setSaving(true);
    try {
      await onSave(value);
      setHasUnsavedChanges(false);
    } catch {
    } finally {
      setSaving(false);
      saveLockRef.current = false;
    }
  }, DEBOUNCE_MS);

  const handleChange = useCallback(
    (view: EditorView, shouldSave: boolean) => {
      const newContent = view.state.doc.toString();
      setContent(newContent);
      if (!shouldSave) return;
      setHasUnsavedChanges(true);
      debouncedSave(newContent);
    },
    [debouncedSave],
  );

  const updateSelectionToolbar = useCallback((view: EditorView) => {
    const selection = view.state.selection.main;
    if (readOnly || currentMode !== "edit" || selection.empty || !container) {
      setSelectionToolbar({ open: false, left: 0, top: 0 });
      return;
    }

    const start = view.coordsAtPos(selection.from);
    const end = view.coordsAtPos(selection.to);
    if (!start || !end) {
      setSelectionToolbar({ open: false, left: 0, top: 0 });
      return;
    }

    const bounds = container.getBoundingClientRect();
    const left = ((start.left + end.right) / 2) - bounds.left;
    const top = Math.max(8, Math.min(start.top, end.top) - bounds.top - 44);

    setSelectionToolbar({
      open: true,
      left,
      top,
    });
  }, [container, currentMode, readOnly]);

  const updateListener = useMemo(
    () =>
      EditorView.updateListener.of((update: ViewUpdate) => {
        if (update.docChanged) handleChange(update.view, !readOnly);
        if (update.docChanged || update.selectionSet || update.focusChanged) {
          updateSelectionToolbar(update.view);
        }
      }),
    [handleChange, readOnly, updateSelectionToolbar],
  );

  const editorKeymap = useMemo(
    () => keymap.of([
      { key: "Mod-b", run: (view) => dispatchEditorAction(view, "bold") },
      { key: "Mod-i", run: (view) => dispatchEditorAction(view, "italic") },
      { key: "Mod-e", run: (view) => dispatchEditorAction(view, "inlineCode") },
      { key: "Mod-Shift-x", run: (view) => dispatchEditorAction(view, "strikethrough") },
      { key: "Mod-Alt-1", run: (view) => dispatchEditorAction(view, "h1") },
      { key: "Mod-Alt-2", run: (view) => dispatchEditorAction(view, "h2") },
      { key: "Mod-Alt-3", run: (view) => dispatchEditorAction(view, "h3") },
      { key: "Mod-Alt-c", run: (view) => dispatchEditorAction(view, "codeBlock") },
      { key: "Mod-Shift-7", run: (view) => dispatchEditorAction(view, "checklist") },
      { key: "Mod-Alt--", run: (view) => dispatchEditorAction(view, "divider") },
    ]),
    [],
  );

  const extensions = useMemo<Extension[]>(
    () => [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      drawSelection(),
      highlightActiveLine(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      bracketMatching(),
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      editorKeymap,
      keymap.of([...defaultKeymap, ...historyKeymap]),
      updateListener,
      EditorView.theme({
        "&": { height: "100%" },
        ".cm-scroller": { overflow: "auto" },
        "&.cm-focused": { outline: "none" },
      }),
    ],
    [editorKeymap, updateListener],
  );

  const collabOptions = useMemo(
    () =>
      container && (currentMode === "edit" || readOnly)
        ? {
            docId,
            container,
            extensions,
            initialContent,
            readOnly,
            onTitleUpdate,
            onViewChange: setEditorView,
          }
        : null,
    [container, currentMode, docId, extensions, initialContent, onTitleUpdate, readOnly],
  );

  useCollabEditor(collabOptions);

  function runToolbarAction(actionId: FormattingActionId) {
    if (editorView) {
      dispatchEditorAction(editorView, actionId);
      updateSelectionToolbar(editorView);
    }
  }

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {!focusMode && (
        <div className="flex min-h-12 items-center gap-2 border-b border-border/80 bg-card px-2 py-2 shadow-xs">
          <div className="flex items-center gap-1">
            <Tooltip>
              <TooltipTrigger
                render={
                  <Toggle
                    pressed={currentMode === "edit"}
                    onPressedChange={() => setMode("edit")}
                    size="sm"
                    className="gap-1.5 text-xs"
                  />
                }
              >
                <Code2 className="size-3.5" />
                Edit
              </TooltipTrigger>
              <TooltipContent>Edit mode</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Toggle
                    pressed={currentMode === "preview"}
                    onPressedChange={() => setMode("preview")}
                    size="sm"
                    className="gap-1.5 text-xs"
                  />
                }
              >
                <Eye className="size-3.5" />
                Preview
              </TooltipTrigger>
              <TooltipContent>Preview mode</TooltipContent>
            </Tooltip>
          </div>

          {mode === "edit" && !readOnly && (
            <div
              className="flex flex-wrap items-center gap-1 border-l border-border/80 pl-2"
              data-testid="editor-toolbar"
            >
              {EDITOR_ACTIONS.map((action) => (
                <Tooltip key={action.id}>
                  <TooltipTrigger
                    render={
                      <Button
                        type="button"
                        variant="ghost"
                        size="xs"
                        onClick={() => runToolbarAction(action.id)}
                        aria-label={action.label}
                      />
                    }
                  >
                    {action.shortLabel}
                  </TooltipTrigger>
                  <TooltipContent>{action.label} · {action.shortcut}</TooltipContent>
                </Tooltip>
              ))}
            </div>
          )}

          <div className="ml-auto flex items-center gap-2">
            {saving && (
              <span className="text-[11px] text-muted-foreground animate-pulse">
                Saving...
              </span>
            )}
            {!saving && hasUnsavedChanges && (
              <span className="text-[11px] text-muted-foreground">Unsaved</span>
            )}
            <AvatarStrip />
            <Tooltip>
              <TooltipTrigger
                render={
                  <div className="flex items-center gap-1.5 rounded-full border border-border/70 bg-card/70 px-2 py-1 text-[11px] text-muted-foreground shadow-xs">
                    <span className={`inline-block size-1.5 rounded-full ${STATUS_DOT[collabStatus]}`} />
                    {STATUS_LABEL[collabStatus]}
                  </div>
                }
              />
              <TooltipContent>
                {collabStatus === "connected"
                  ? "Connected to server"
                  : collabStatus === "disconnected"
                    ? "Changes saved locally — will sync when reconnected"
                    : STATUS_LABEL[collabStatus]}
              </TooltipContent>
            </Tooltip>
          </div>
        </div>
      )}

      {currentMode === "preview" ? (
        <>
          <div
            className="prose prose-sm dark:prose-invert prose-headings:font-heading mx-auto w-full max-w-3xl flex-1 overflow-y-auto bg-card p-8"
            dangerouslySetInnerHTML={{ __html: sanitizeMarkdownPreview(content) }}
          />
          {readOnly && (
            <div
              ref={setContainer}
              className={cn("hidden", fontClassName)}
              data-testid="editor-container"
            />
          )}
        </>
      ) : (
        <div className="relative flex-1 overflow-hidden bg-card">
          {currentMode === "edit" && selectionToolbar.open && (
            <div
              className="absolute z-20 flex -translate-x-1/2 items-center gap-1 rounded-lg border border-border/80 bg-popover/95 p-1 shadow-lg shadow-foreground/10 ring-1 ring-primary/10 backdrop-blur"
              style={{ left: selectionToolbar.left, top: selectionToolbar.top }}
              data-testid="selection-toolbar"
            >
              {EDITOR_ACTIONS.map((action) => (
                <Button
                  key={action.id}
                  type="button"
                  variant="ghost"
                  size="xs"
                  onClick={() => runToolbarAction(action.id)}
                  aria-label={`Selection ${action.label}`}
                >
                  {action.shortLabel}
                </Button>
              ))}
            </div>
          )}
          <div
            ref={setContainer}
            className={cn("cm-editor-container flex-1 overflow-hidden", fontClassName)}
            data-testid="editor-container"
          />
        </div>
      )}
    </div>
  );
}
