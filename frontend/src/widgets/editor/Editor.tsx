import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useCollabStore } from "#/features/collab/store";
import { useCollabEditor } from "#/features/collab/useCollabEditor";
import type { EditorView, ViewUpdate } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { Button } from "#/components/ui/button";
import { EDITOR_ACTIONS } from "./editorActions";
import { getFormattingEdit, type FormattingActionId } from "./formatting";
import { cn } from "#/lib/utils";
import MarkdownIt from "markdown-it";
import EditorHeader from "./EditorHeader";

interface CodeMirrorModules {
  view: typeof import("@codemirror/view");
  commands: typeof import("@codemirror/commands");
  language: typeof import("@codemirror/language");
  markdown: typeof import("@codemirror/lang-markdown");
  languageData: typeof import("@codemirror/language-data");
}

interface EditorProps {
  docId: string;
  initialContent: string;
  onTitleUpdate?: (title: string) => void;
  focusMode?: boolean;
  fontClassName?: string;
  readOnly?: boolean;
}

function sanitizeMarkdownPreview(markdown: string) {
  return new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true,
  }).render(markdown);
}

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
  onTitleUpdate,
  focusMode = false,
  fontClassName = "font-sans",
  readOnly = false,
}: EditorProps) {
  const collabStatus = useCollabStore((s) => s.status);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const currentMode = readOnly ? "preview" : mode;
  const currentModeRef = useRef(currentMode);
  currentModeRef.current = currentMode;
  const [content, setContent] = useState(initialContent);

  const [editorView, setEditorView] = useState<EditorView | null>(null);
  const [selectionToolbar, setSelectionToolbar] = useState<{
    open: boolean;
    left: number;
    top: number;
  }>({ open: false, left: 0, top: 0 });
  const [codeMirror, setCodeMirror] = useState<CodeMirrorModules | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      import("@codemirror/view"),
      import("@codemirror/commands"),
      import("@codemirror/language"),
      import("@codemirror/lang-markdown"),
      import("@codemirror/language-data"),
    ]).then(([view, commands, language, markdown, languageData]) => {
      if (!cancelled) {
        setCodeMirror({ view, commands, language, markdown, languageData });
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleChange = useCallback((view: EditorView) => {
    setContent(view.state.doc.toString());
  }, []);

  const updateSelectionToolbar = useCallback((view: EditorView) => {
    const selection = view.state.selection.main;
    if (readOnly || currentModeRef.current !== "edit" || selection.empty || !container) {
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
  }, [container, readOnly]);

  const updateListener = useMemo(() => {
    if (!codeMirror) return null;
    return codeMirror.view.EditorView.updateListener.of((update: ViewUpdate) => {
      if (update.docChanged) handleChange(update.view);
      if (update.docChanged || update.selectionSet || update.focusChanged) {
        updateSelectionToolbar(update.view);
      }
    });
  }, [codeMirror, handleChange, updateSelectionToolbar]);

  const editorKeymap = useMemo(() => {
    if (!codeMirror) return null;
    return codeMirror.view.keymap.of([
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
    ]);
  }, [codeMirror]);

  const extensions = useMemo<Extension[]>(() => {
    if (!codeMirror || !editorKeymap || !updateListener) return [];
    return [
      codeMirror.view.lineNumbers(),
      codeMirror.view.highlightActiveLineGutter(),
      codeMirror.view.highlightSpecialChars(),
      codeMirror.commands.history(),
      codeMirror.view.drawSelection(),
      codeMirror.view.highlightActiveLine(),
      codeMirror.language.syntaxHighlighting(codeMirror.language.defaultHighlightStyle, { fallback: true }),
      codeMirror.language.bracketMatching(),
      codeMirror.markdown.markdown({
        base: codeMirror.markdown.markdownLanguage,
        codeLanguages: codeMirror.languageData.languages,
      }),
      editorKeymap,
      codeMirror.view.keymap.of([
        ...codeMirror.commands.defaultKeymap,
        ...codeMirror.commands.historyKeymap,
      ]),
      updateListener,
      codeMirror.view.EditorView.theme({
        "&": { height: "100%" },
        ".cm-scroller": { overflow: "auto" },
        "&.cm-focused": { outline: "none" },
      }),
    ];
  }, [codeMirror, editorKeymap, updateListener]);

  const collabOptions = useMemo(
    () =>
      codeMirror && container
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
    [codeMirror, container, docId, extensions, initialContent, onTitleUpdate, readOnly],
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
        <EditorHeader
          mode={currentMode}
          readOnly={readOnly}
          collabStatus={collabStatus}
          onModeChange={setMode}
          onToolbarAction={runToolbarAction}
        />
      )}

      {currentMode === "preview" && (
        <div
          className="prose prose-sm dark:prose-invert prose-headings:font-heading mx-auto w-full max-w-3xl flex-1 overflow-y-auto bg-card p-8"
          dangerouslySetInnerHTML={{ __html: sanitizeMarkdownPreview(content) }}
        />
      )}
      <div
        className={cn(
          "relative flex-1 overflow-hidden bg-card",
          currentMode === "preview" && "hidden",
        )}
      >
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
    </div>
  );
}
