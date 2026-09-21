/**
 * Regression tests for the editor audit (docs/BUG_AUDIT.md #8, #9, #10, #15).
 *
 * The real Editor component, its configured extensions, undo keymap and
 * preview rendering are exercised. Only the network hook is
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

async function mount() {
  const ui = render(<Editor docId="audit" initialContent="hello" />);
  await waitFor(() => expect(h.views.length).toBeGreaterThan(0));
  return { ...ui, view: h.views.at(-1), doc: h.docs.at(-1) };
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

it("REG-08: collaboration keybindings undo and redo only local edits", async () => {
  const { view, doc } = await mount();
  act(() => view.dispatch({ changes: { from: 5, insert: " local" } }));
  const remote = new Y.Doc();
  Y.applyUpdate(remote, Y.encodeStateAsUpdate(doc));
  remote.getText("content").insert(0, "remote ");
  act(() => Y.applyUpdate(doc, Y.encodeStateAsUpdate(remote), remote));

  const isMac = /Mac/.test(navigator.platform);
  fireEvent.keyDown(view.contentDOM, {
    key: "z",
    ...(isMac ? { metaKey: true } : { ctrlKey: true }),
  });
  await waitFor(() =>
    expect(view.state.doc.toString()).toBe("remote hello"),
  );
  expect(doc.getText("content").toString()).toBe("remote hello");

  fireEvent.keyDown(view.contentDOM, {
    key: isMac ? "z" : "y",
    shiftKey: isMac,
    ...(isMac ? { metaKey: true } : { ctrlKey: true }),
  });
  await waitFor(() =>
    expect(view.state.doc.toString()).toBe("remote hello local"),
  );
  expect(doc.getText("content").toString()).toBe("remote hello local");
  remote.destroy();
});

it("REG-09: rapid edits stay in the shared CRDT without plaintext autosaves", async () => {
  const { view, doc } = await mount();
  act(() => view.dispatch({ changes: { from: 5, insert: " A" } }));
  act(() => view.dispatch({ changes: { from: 7, insert: " B" } }));
  expect(view.state.doc.toString()).toBe("hello A B");
  expect(doc.getText("content").toString()).toBe("hello A B");
});

it("REG-10: navigating away schedules no plaintext final save", async () => {
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  const ui = await mount();
  vi.useFakeTimers();
  act(() => ui.view.dispatch({ changes: { from: 5, insert: " durable" } }));
  ui.unmount();
  await act(async () => {
    await vi.runAllTimersAsync();
  });
  expect(fetch).not.toHaveBeenCalled();
});

it("REG-15: preview mode keeps receiving collaborators' edits", async () => {
  const ui = await mount();
  fireEvent.click(ui.getByRole("button", { name: "Preview" }));
  await waitFor(() => expect(ui.container.textContent).toContain("hello"));
  act(() => ui.doc.getText("content").insert(5, " remote"));
  await waitFor(() => expect(ui.container.textContent).toContain("remote"));
});

it("REG-15: switching through preview retains the collaboration session", async () => {
  const ui = await mount();
  fireEvent.click(ui.getByRole("button", { name: "Preview" }));
  act(() => ui.doc.getText("content").insert(5, " remote"));
  await waitFor(() => expect(ui.container.textContent).toContain("remote"));
  fireEvent.click(ui.getByRole("button", { name: "Edit" }));
  await waitFor(() => expect(ui.view.state.doc.toString()).toBe("hello remote"));
  expect(h.docs).toHaveLength(1);
  expect(h.views).toHaveLength(1);
});
