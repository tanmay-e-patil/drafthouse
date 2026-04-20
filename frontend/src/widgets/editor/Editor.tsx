import { useCallback, useEffect, useRef, useState } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import type { ViewUpdate } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { useDebounce } from "./useDebounce";

interface EditorProps {
  initialContent: string;
  onSave: (content: string) => Promise<void>;
}

const DEBOUNCE_MS = 500;

export default function Editor({ initialContent, onSave }: EditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [content, setContent] = useState(initialContent);
  const [saving, setSaving] = useState(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [previewHtml, setPreviewHtml] = useState("");
  const [loading, setLoading] = useState(true);
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
    () => {
      if (!viewRef.current) return;
      const newContent = viewRef.current.state.doc.toString();
      setContent(newContent);
      setHasUnsavedChanges(true);
      debouncedSave(newContent);
    },
    [debouncedSave]
  );

  const isEditMode = mode === "edit";

  useEffect(() => {
    if (!isEditMode) return;
    if (!editorRef.current) return;

    // Re-create editor if destroyed (viewRef was nulled by cleanup)
    // We check if the DOM has no .cm-editor child as a proxy for "needs re-creation"
    const needsInit = !editorRef.current.querySelector(".cm-editor");

    if (!needsInit) {
      // Already has editor, just focus
      viewRef.current?.focus();
      return;
    }

    const updateListener = EditorView.updateListener.of((update: ViewUpdate) => {
      if (update.docChanged) {
        handleChange();
      }
    });

    const extensions: Extension[] = [
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
    ];

    const state = EditorState.create({
      doc: content,
      extensions,
    });

    const view = new EditorView({
      state,
      parent: editorRef.current,
    });

    viewRef.current = view;
    setLoading(false);

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [isEditMode, handleChange]);

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

  useEffect(() => {
    if (viewRef.current) {
      viewRef.current.focus();
    }
  }, [loading]);

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
      </div>
      <div ref={editorRef} className="cm-editor-container" />
    </div>
  );
}
