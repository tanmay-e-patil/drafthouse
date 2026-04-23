import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

class MockEditorView {
  focus = vi.fn();
  destroy = vi.fn();
  static updateListener = { of: vi.fn().mockReturnValue([]) };
  static theme = vi.fn().mockReturnValue([]);
  state = { doc: { toString: () => "# Hello" } };

  constructor(_opts: { parent: HTMLElement | null }) {}
}

vi.mock("@codemirror/view", () => ({
  EditorView: MockEditorView,
  keymap: { of: vi.fn().mockReturnValue([]) },
  lineNumbers: vi.fn(() => []),
  highlightActiveLineGutter: vi.fn(() => []),
  highlightSpecialChars: vi.fn(() => []),
  drawSelection: vi.fn(() => []),
  highlightActiveLine: vi.fn(() => []),
}));

vi.mock("@codemirror/state", () => ({
  EditorState: {
    create: vi.fn(() => ({ doc: "# Hello" })),
    allowMultipleSelections: { of: vi.fn() },
  },
}));

vi.mock("@codemirror/commands", () => ({
  defaultKeymap: [],
  history: vi.fn(() => []),
  historyKeymap: [],
}));

vi.mock("@codemirror/language", () => ({
  syntaxHighlighting: vi.fn(() => []),
  defaultHighlightStyle: {},
  bracketMatching: vi.fn(() => []),
}));

vi.mock("@codemirror/lang-markdown", () => ({
  markdown: vi.fn(() => []),
  markdownLanguage: {},
}));

vi.mock("@codemirror/language-data", () => ({
  languages: [],
}));

class MockMarkdownIt {
  render(md: string) { return `<h1>${md.replace("# ", "")}</h1>`; }
}

vi.mock("markdown-it", () => ({
  default: MockMarkdownIt,
}));

vi.mock("../useDebounce", () => ({
  useDebounce: () => vi.fn(),
}));

describe("Editor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("accepts onTitleUpdate prop without error", async () => {
    const { default: Editor } = await import("../Editor");
    const onSave = vi.fn().mockResolvedValue(undefined);
    const onTitleUpdate = vi.fn();

    // Should render without errors when onTitleUpdate is provided
    const { unmount } = render(
      <Editor
        docId="test-doc-id"
        initialContent="# Hello"
        onSave={onSave}
        onTitleUpdate={onTitleUpdate}
      />
    );
    await waitFor(() => {
      expect(screen.getByText("Edit")).toBeDefined();
    });
    unmount();
  });

  it("renders edit mode by default with toolbar buttons", async () => {
    const { default: Editor } = await import("../Editor");
    const onSave = vi.fn().mockResolvedValue(undefined);

    render(<Editor docId="test-doc-id" initialContent="# Hello" onSave={onSave} />);

    await waitFor(() => {
      expect(screen.getByText("Edit")).toBeDefined();
      expect(screen.getByText("Preview")).toBeDefined();
      expect(screen.getByLabelText("Heading 1")).toBeDefined();
      expect(screen.getByLabelText("Bold")).toBeDefined();
      expect(screen.getByLabelText("Inline code")).toBeDefined();
    });
  });

  it("switches to preview mode on Preview button click", async () => {
    const { default: Editor } = await import("../Editor");
    const onSave = vi.fn().mockResolvedValue(undefined);

    render(<Editor docId="test-doc-id" initialContent="# Hello" onSave={onSave} />);

    await waitFor(() => {
      expect(screen.getByText("Preview")).toBeDefined();
    });

    fireEvent.click(screen.getByText("Preview"));

    await waitFor(() => {
      expect(screen.getByText("Edit")).toBeDefined();
      expect(screen.getByText("Preview")).toBeDefined();
    });
  });

  it("switches back to edit mode from preview", async () => {
    const { default: Editor } = await import("../Editor");
    const onSave = vi.fn().mockResolvedValue(undefined);

    render(<Editor docId="test-doc-id" initialContent="# Hello" onSave={onSave} />);

    await waitFor(() => {
      expect(screen.getByText("Preview")).toBeDefined();
    });

    fireEvent.click(screen.getByText("Preview"));

    await waitFor(() => {
      expect(screen.getByText("Edit")).toBeDefined();
    });

    fireEvent.click(screen.getByText("Edit"));

    await waitFor(() => {
      expect(screen.getByText("Preview")).toBeDefined();
    });
  });
});
