import type { FormattingActionId } from "./formatting";

export interface EditorAction {
  id: FormattingActionId;
  label: string;
  shortLabel: string;
  shortcut: string;
}

export const EDITOR_ACTIONS: EditorAction[] = [
  { id: "h1", label: "Heading 1", shortLabel: "H1", shortcut: "Cmd/Ctrl+Alt+1" },
  { id: "h2", label: "Heading 2", shortLabel: "H2", shortcut: "Cmd/Ctrl+Alt+2" },
  { id: "h3", label: "Heading 3", shortLabel: "H3", shortcut: "Cmd/Ctrl+Alt+3" },
  { id: "bold", label: "Bold", shortLabel: "B", shortcut: "Cmd/Ctrl+B" },
  { id: "italic", label: "Italic", shortLabel: "I", shortcut: "Cmd/Ctrl+I" },
  { id: "strikethrough", label: "Strikethrough", shortLabel: "S", shortcut: "Cmd/Ctrl+Shift+X" },
  { id: "inlineCode", label: "Inline code", shortLabel: "` `", shortcut: "Cmd/Ctrl+E" },
  { id: "codeBlock", label: "Code block", shortLabel: "{ }", shortcut: "Cmd/Ctrl+Alt+C" },
  { id: "checklist", label: "Checklist", shortLabel: "[ ]", shortcut: "Cmd/Ctrl+Shift+7" },
  { id: "divider", label: "Divider", shortLabel: "---", shortcut: "Cmd/Ctrl+Alt+-" },
];
