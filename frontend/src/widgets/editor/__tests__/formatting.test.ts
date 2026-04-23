import { describe, expect, it } from "vitest";
import {
  applyFormattingEdit,
  getFormattingEdit,
} from "../formatting";

describe("formatting helpers", () => {
  it("wraps selected text in bold markers", () => {
    const text = "hello world";
    const edit = getFormattingEdit("bold", text, { from: 6, to: 11 });

    expect(applyFormattingEdit(text, edit)).toBe("hello **world**");
    expect(edit.selection).toEqual({ from: 8, to: 13 });
  });

  it("inserts inline markers around an empty selection", () => {
    const text = "hello";
    const edit = getFormattingEdit("italic", text, { from: 5, to: 5 });

    expect(applyFormattingEdit(text, edit)).toBe("hello**");
    expect(edit.selection).toEqual({ from: 6, to: 6 });
  });

  it("rewrites heading prefixes instead of stacking them", () => {
    const text = "# Existing";
    const edit = getFormattingEdit("h3", text, { from: 2, to: 10 });

    expect(applyFormattingEdit(text, edit)).toBe("### Existing");
  });

  it("wraps selections in fenced code blocks", () => {
    const text = "const x = 1;";
    const edit = getFormattingEdit("codeBlock", text, { from: 0, to: text.length });

    expect(applyFormattingEdit(text, edit)).toBe("```\nconst x = 1;\n```");
  });

  it("adds checklist markers to the current line", () => {
    const text = "Buy milk";
    const edit = getFormattingEdit("checklist", text, { from: 0, to: 8 });

    expect(applyFormattingEdit(text, edit)).toBe("- [ ] Buy milk");
  });

  it("inserts a standalone divider block", () => {
    const text = "Hello\nWorld";
    const edit = getFormattingEdit("divider", text, { from: 6, to: 6 });

    expect(applyFormattingEdit(text, edit)).toBe("Hello\n\n---\n\nWorld");
  });
});
