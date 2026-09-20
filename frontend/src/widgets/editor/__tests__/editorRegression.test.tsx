/**
 * Regression tests for the editor audit (docs/BUG_AUDIT.md #8, #9, #10, #15).
 *
 * The real Editor component, its configured extensions, undo keymap, debounce,
 * save lock and preview rendering are exercised. Only the network hook is
 * substituted with a local Yjs/CodeMirror binding so editor behavior can be
 * tested without a server.
 */
import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import * as Y from "yjs";
import { undo } from "@codemirror/commands";
import Editor from "../Editor";

// jsdom has no layout engine; CodeMirror needs this minimal geometry.
Range.prototype.getClientRects = function () {
  return [] as unknown as DOMRectList;
};
Range.prototype.getBoundingClientRect = function () {
  return new DOMRect();
};

const h = vi.hoisted(() => ({ views: [] as any[], docs: [] as any[] }));
vi.mock("#/features/collab/useCollabEditor", async () => {
  const { useEffect, useRef } = await import("react");
  const { EditorView } = await import("@codemirror/view");
  const { EditorState } = await import("@codemirror/state");
  const { yCollab } = await import("y-codemirror.next");
  const Y = await import("yjs");
  return {
    useCollabEditor(options: any) {
      useEffect(() => {
        if (!options) return;
        const doc = new Y.Doc();
        const text = doc.getText("content");
        text.insert(0, options.initialContent);
        const view = new EditorView({
          parent: options.container,
          state: EditorState.create({
            doc: text.toString(),
            extensions: [...options.extensions, yCollab(text, null)],
          }),
        });
        h.views.push(view);
        h.docs.push(doc);
        options.onViewChange?.(view);
        return () => {
          view.destroy();
          options.onViewChange?.(null);
        };
      }, [options]);
      return useRef(null);
    },
  };
});

afterEach(() => {
  cleanup();
  h.docs.forEach((d) => d.destroy());
  h.views.length = 0;
  h.docs.length = 0;
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

async function mount(onSave = vi.fn().mockResolvedValue(undefined)) {
  const ui = render(<Editor docId="audit" initialContent="hello" onSave={onSave} />);
  await waitFor(() => expect(h.views.length).toBeGreaterThan(0));
  return { ...ui, onSave, view: h.views.at(-1), doc: h.docs.at(-1) };
}

it("REG-08: undo never removes another collaborator's change", async () => {
  const { view, doc } = await mount();
  const remote = new Y.Doc();
  Y.applyUpdate(remote, Y.encodeStateAsUpdate(doc));
  remote.getText("content").insert(5, " remote");
  act(() => Y.applyUpdate(doc, Y.encodeStateAsUpdate(remote)));
  expect(view.state.doc.toString()).toBe("hello remote");
  act(() => undo(view));
  expect(doc.getText("content").toString()).toBe("hello remote");
  expect(view.state.doc.toString()).toBe("hello remote");
  remote.destroy();
});

it("REG-09: edits made during an in-flight save are persisted afterwards", async () => {
  let finish!: () => void;
  const save = vi.fn(() => new Promise<void>((r) => { finish = r; }));
  const { view } = await mount(save);
  vi.useFakeTimers();
  act(() => view.dispatch({ changes: { from: 5, insert: " A" } }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(save).toHaveBeenCalledWith("hello A");
  act(() => view.dispatch({ changes: { from: 7, insert: " B" } }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  await act(async () => {
    finish();
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1000);
  });
  expect(view.state.doc.toString()).toBe("hello A B");
  const calls = save.mock.calls.map((c: unknown[]) => c[0]);
  expect(calls[calls.length - 1]).toBe("hello A B");
});

it("REG-10: navigating away flushes the pending final save", async () => {
  const ui = await mount();
  vi.useFakeTimers();
  act(() => ui.view.dispatch({ changes: { from: 5, insert: " unsaved" } }));
  ui.unmount();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(ui.onSave).toHaveBeenCalledWith("hello unsaved");
});

it("REG-15: preview mode keeps receiving collaborators' edits", async () => {
  const ui = await mount();
  fireEvent.click(ui.getByRole("button", { name: "Preview" }));
  await waitFor(() => expect(ui.container.textContent).toContain("hello"));
  act(() => ui.doc.getText("content").insert(5, " remote"));
  await waitFor(() => expect(ui.container.textContent).toContain("remote"));
});
