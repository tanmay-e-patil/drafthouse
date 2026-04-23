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
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "#/components/ui/tooltip";
import { Eye, Code2 } from "lucide-react";

interface EditorProps {
  docId: string;
  initialContent: string;
  onSave: (content: string) => Promise<void>;
  onTitleUpdate?: (title: string) => void;
}

const DEBOUNCE_MS = 500;

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  connecting: "Connecting...",
  connected: "Synced",
  syncing: "Syncing...",
  disconnected: "Offline",
};

const STATUS_DOT: Record<ConnectionStatus, string> = {
  connecting: "bg-yellow-500",
  connected: "bg-green-500",
  syncing: "bg-yellow-500",
  disconnected: "bg-red-500",
};

export default function Editor({ docId, initialContent, onSave, onTitleUpdate }: EditorProps) {
  const collabStatus = useCollabStore((s) => s.status);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [content, setContent] = useState(initialContent);
  const [saving, setSaving] = useState(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [previewHtml, setPreviewHtml] = useState("");
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
    (view: EditorView) => {
      const newContent = view.state.doc.toString();
      setContent(newContent);
      setHasUnsavedChanges(true);
      debouncedSave(newContent);
    },
    [debouncedSave],
  );

  const updateListener = useMemo(
    () =>
      EditorView.updateListener.of((update: ViewUpdate) => {
        if (update.docChanged) handleChange(update.view);
      }),
    [handleChange],
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
      keymap.of([...defaultKeymap, ...historyKeymap]),
      updateListener,
      EditorView.theme({
        "&": { height: "100%" },
        ".cm-scroller": { overflow: "auto" },
        "&.cm-focused": { outline: "none" },
      }),
    ],
    [updateListener],
  );

  useCollabEditor(
    container && mode === "edit"
      ? { docId, container, extensions, initialContent, onTitleUpdate }
      : null,
  );

  useEffect(() => {
    if (mode === "preview") {
      import("markdown-it").then((mod) => {
        const md = new mod.default({
          html: true,
          linkify: true,
          typographer: true,
        });
        setPreviewHtml(md.render(content));
      });
    }
  }, [mode, content]);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex h-10 items-center gap-1 border-b border-border px-2">
        <Tooltip>
          <TooltipTrigger
            render={
              <Toggle
                pressed={mode === "edit"}
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
                pressed={mode === "preview"}
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
                <div className="flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-[11px] text-muted-foreground">
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

      {mode === "preview" ? (
        <div
          className="prose prose-sm dark:prose-invert max-w-none flex-1 overflow-y-auto p-6"
          dangerouslySetInnerHTML={{ __html: previewHtml }}
        />
      ) : (
        <div ref={setContainer} className="flex-1 overflow-hidden" />
      )}
    </div>
  );
}
