import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useCollabStore, type ConnectionStatus } from "#/features/collab/store";
import { useCollabEditor } from "#/features/collab/useCollabEditor";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import type { ViewUpdate } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { useDebounce } from "./useDebounce";

interface EditorProps {
  docId: string;
  initialContent: string;
  onSave: (content: string) => Promise<void>;
}

const DEBOUNCE_MS = 500;

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  connecting: "Connecting...",
  connected: "Synced",
  syncing: "Syncing...",
  disconnected: "Working offline",
};

const STATUS_COLOR: Record<ConnectionStatus, string> = {
  connecting: "var(--color-yellow-500, #eab308)",
  connected: "var(--color-green-500, #22c55e)",
  syncing: "var(--color-yellow-500, #eab308)",
  disconnected: "var(--color-red-500, #ef4444)",
};

export default function Editor({ docId, initialContent, onSave }: EditorProps) {
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
      // keep unsaved state
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
    [debouncedSave]
  );

  const updateListener = useMemo(
    () =>
      EditorView.updateListener.of((update: ViewUpdate) => {
        if (update.docChanged) handleChange(update.view);
      }),
    [handleChange]
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
    [updateListener]
  );

  useCollabEditor(
    container && mode === "edit" ? { docId, container, extensions } : null
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

  if (mode === "preview") {
    return (
      <div className="editor-container">
        <div className="editor-toolbar">
          <button
            className="toolbar-btn"
            onClick={() => setMode("edit")}
          >
            Edit
          </button>
          <button className="toolbar-btn active" disabled>
            Preview
          </button>
        </div>
        <div
          className="markdown-preview"
          dangerouslySetInnerHTML={{ __html: previewHtml }}
        />
      </div>
    );
  }

  return (
    <div className="editor-container">
      <div className="editor-toolbar">
        <button className="toolbar-btn active" disabled>
          Edit
        </button>
        <button
          className="toolbar-btn"
          onClick={() => setMode("preview")}
        >
          Preview
        </button>
        {saving && <span className="save-indicator saving">Saving...</span>}
        {!saving && hasUnsavedChanges && <span className="save-indicator unsaved">Unsaved</span>}
        <span className="connection-status" title={STATUS_LABEL[collabStatus]}>
          <span
            style={{
              display: "inline-block",
              width: 8,
              height: 8,
              borderRadius: "50%",
              backgroundColor: STATUS_COLOR[collabStatus],
              marginRight: 4,
            }}
          />
          {STATUS_LABEL[collabStatus]}
        </span>
      </div>
      <div ref={setContainer} className="cm-editor-container" />
    </div>
  );
}
