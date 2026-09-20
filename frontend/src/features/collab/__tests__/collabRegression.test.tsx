/**
 * Regression tests for the collaboration audit (docs/BUG_AUDIT.md).
 *
 * Each REG-XX test asserts the REQUIRED behavior for audit finding #XX.
 * They are intentionally red while the underlying bug is unfixed; each fix
 * must turn its test green without weakening the assertion.
 *
 * Real React hook, Yjs, y-websocket provider, CodeMirror editor, stores and
 * avatar UI are exercised. Only the browser WebSocket transport and the
 * ticket endpoint are substituted, mirroring the y-websocket server protocol.
 */
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, render, renderHook, waitFor } from "@testing-library/react";
import * as Y from "yjs";
import * as encoding from "lib0/encoding";
import * as decoding from "lib0/decoding";
import { EditorView } from "@codemirror/view";
import { ySyncFacet } from "y-codemirror.next";
import { useCollabEditor } from "#/features/collab/useCollabEditor";
import { useAuthStore } from "#/features/auth/store";
import { useAwarenessStore } from "#/features/collab/awarenessStore";
import AvatarStrip from "#/features/collab/ui/AvatarStrip";

// jsdom has no layout engine; CodeMirror needs this minimal geometry.
Range.prototype.getClientRects = function () {
  return [] as unknown as DOMRectList;
};
Range.prototype.getBoundingClientRect = function () {
  return new DOMRect();
};

const h = vi.hoisted(() => ({ providers: [] as any[], ticket: vi.fn() }));
vi.mock("#/features/collab/api", () => ({ issueWsTicket: h.ticket }));
vi.mock("y-websocket", async (original) => {
  const real = await original<typeof import("y-websocket")>();
  return {
    ...real,
    WebsocketProvider: class extends real.WebsocketProvider {
      constructor(url: string, id: string, doc: Y.Doc, options: any) {
        super(url, id, doc, { ...options, disableBc: true });
        h.providers.push(this);
      }
    },
  };
});

class Socket {
  static OPEN = 1;
  OPEN = 1;
  readyState = 0;
  sent: Uint8Array[] = [];
  onopen: any;
  onclose: any;
  onmessage: any;
  onerror: any;
  send(data: Uint8Array) {
    this.sent.push(data);
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    queueMicrotask(() => this.onclose?.({}));
  }
  receive(data: Uint8Array) {
    this.onmessage?.({ data });
  }
}

function step2(doc: Y.Doc, vector?: Uint8Array) {
  const e = encoding.createEncoder();
  encoding.writeVarUint(e, 0);
  encoding.writeVarUint(e, 1);
  encoding.writeVarUint8Array(e, Y.encodeStateAsUpdate(doc, vector));
  return encoding.toUint8Array(e);
}

function awarenessFrame(clientId: number, clock: number, state: unknown) {
  const a = encoding.createEncoder();
  encoding.writeVarUint(a, 1);
  encoding.writeVarUint(a, clientId);
  encoding.writeVarUint(a, clock);
  encoding.writeVarString(a, JSON.stringify(state));
  const e = encoding.createEncoder();
  encoding.writeVarUint(e, 1);
  encoding.writeVarUint8Array(e, encoding.toUint8Array(a));
  return encoding.toUint8Array(e);
}

function options(extra = {}) {
  const container = document.createElement("div");
  document.body.append(container);
  return { docId: "audit-doc", container, initialContent: "original", ...extra };
}

async function mount(extra = {}) {
  let view: EditorView | null = null;
  const opts = options({
    ...extra,
    onViewChange: (v: EditorView | null) => {
      view = v;
    },
  });
  const before = h.providers.length;
  const hook = renderHook(() => useCollabEditor(opts));
  await waitFor(() => expect(h.providers.length).toBe(before + 1));
  await waitFor(() => expect(view).not.toBeNull());
  const provider = h.providers[before];
  return { hook, provider, view: view!, opts };
}

async function settle(ms = 50) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

beforeEach(() => {
  vi.stubGlobal("WebSocket", Socket);
  useAuthStore.setState({ accessToken: null, email: "alice@example.com" });
  useAwarenessStore.setState({ peers: [], localClientId: null });
  h.ticket.mockReset();
});

afterEach(() => {
  cleanup();
  for (const p of h.providers) {
    p.destroy();
    p.awareness.destroy();
    p.doc.destroy();
  }
  h.providers.length = 0;
  document.body.innerHTML = "";
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("REG-03: reconnect handshake uploads edits made while disconnected", async () => {
  const { provider: p } = await mount({ initialContent: "" });
  p.doc.getText("content").insert(0, "offline edit");
  const ws: Socket = p.ws;
  const server = new Y.Doc();
  act(() => {
    ws.open();
    ws.receive(step2(server));
  });
  let uploads = 0;
  for (const message of [...ws.sent]) {
    const d = decoding.createDecoder(message);
    if (decoding.readVarUint(d) !== 0) continue;
    const step = decoding.readVarUint(d);
    const payload = decoding.readVarUint8Array(d);
    if (step === 0) act(() => ws.receive(step2(server, payload)));
    else {
      uploads++;
      Y.applyUpdate(server, payload);
    }
  }
  expect(p.synced).toBe(true);
  expect(uploads).toBeGreaterThanOrEqual(1);
  expect(server.getText("content").toString()).toBe("offline edit");
  server.destroy();
});

it("REG-05: two clients joining an empty room seed content exactly once", async () => {
  const a = await mount();
  const b = await mount();
  const server = new Y.Doc();
  act(() => {
    a.provider.ws.open();
    b.provider.ws.open();
    a.provider.ws.receive(step2(server));
    b.provider.ws.receive(step2(server));
  });
  Y.applyUpdate(server, Y.encodeStateAsUpdate(a.provider.doc));
  Y.applyUpdate(server, Y.encodeStateAsUpdate(b.provider.doc));
  expect(server.getText("content").toString()).toBe("original");
  server.destroy();
});

it("REG-05: intentionally cleared content stays cleared; read-only clients never seed", async () => {
  const { provider: p } = await mount({ readOnly: true });
  const server = new Y.Doc();
  act(() => {
    p.ws.open();
    p.ws.receive(step2(server));
  });
  expect(p.doc.getText("content").toString()).toBe("");
  act(() => {
    const t = p.doc.getText("content");
    t.delete(0, t.length);
    p.synced = false;
    p.ws.receive(step2(server));
  });
  expect(p.doc.getText("content").toString()).toBe("");
  server.destroy();
});

it("REG-11: unmounting during pending ticket setup creates no orphan session", async () => {
  useAuthStore.setState({ accessToken: "audit-token" });
  let resolve!: (value: unknown) => void;
  h.ticket.mockReturnValue(new Promise((r) => { resolve = r; }));
  const opts = options();
  const hook = renderHook(() => useCollabEditor(opts));
  hook.unmount();
  await act(async () => {
    resolve({ ticket: "audit-ticket" });
  });
  await settle();
  expect(h.providers).toHaveLength(0);
  expect(opts.container.querySelectorAll(".cm-editor")).toHaveLength(0);
});

it("REG-12: callback identity changes preserve the collaboration session", async () => {
  const opts = options();
  const hook = renderHook(
    ({ callback }) => useCollabEditor({ ...opts, onTitleUpdate: callback }),
    { initialProps: { callback: () => {} } },
  );
  await waitFor(() => expect(h.providers).toHaveLength(1));
  const first = h.providers[0];
  const firstDoc = first.doc;
  hook.rerender({ callback: () => {} });
  hook.rerender({ callback: () => {} });
  await settle();
  expect(h.providers).toHaveLength(1);
  expect(h.providers[0]).toBe(first);
  expect(h.providers[0].doc).toBe(firstDoc);
});

it("REG-13: the editor binding tracks the awareness of the current provider", async () => {
  const { provider: first, view } = await mount();
  vi.useFakeTimers();
  act(() => {
    first.ws.open();
    first.ws.close();
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1500);
  });
  const current = h.providers.at(-1)!;
  expect(view.state.facet(ySyncFacet).awareness).toBe(current.awareness);
});

it("REG-14: a successful built-in reconnection is not torn down by hook timers", async () => {
  const { provider: first } = await mount();
  vi.spyOn(Math, "random").mockReturnValue(0.5);
  vi.useFakeTimers();
  act(() => {
    first.ws.open();
    first.ws.close();
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(150);
  });
  act(() => first.ws.open()); // library retry has recovered
  expect(first.wsconnected).toBe(true);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2000);
  });
  expect(first.shouldConnect).toBe(true);
  expect(h.providers).toHaveLength(1);
});

it("REG-17: a failed ticket request never falls back to an unauthenticated socket", async () => {
  useAuthStore.setState({ accessToken: "expired-token" });
  h.ticket.mockRejectedValue(new Error("401"));
  renderHook(() => useCollabEditor(options()));
  await settle(200);
  expect(h.providers).toHaveLength(0);
});

it("REG-20: peers with identical display names remain distinct entries", async () => {
  const { provider: p } = await mount();
  const user = { name: "bob", color: "#123456", lastActive: Date.now() };
  act(() => {
    p.ws.open();
    p.ws.receive(awarenessFrame(77, 1, { user }));
    p.ws.receive(awarenessFrame(88, 1, { user }));
  });
  expect(p.awareness.getStates().size).toBe(3);
  expect(useAwarenessStore.getState().peers.filter((x) => x.name === "bob")).toHaveLength(2);
});

it("REG-SELF: another session of the local user is not shown as a collaborator avatar", async () => {
  const { provider: p } = await mount();
  act(() => {
    p.ws.open();
    p.ws.receive(
      awarenessFrame(77, 1, {
        user: { name: "alice", color: "#123456", lastActive: Date.now() + 1 },
      }),
    );
  });
  const ui = render(<AvatarStrip />);
  expect(ui.queryByTitle("alice")).toBeNull();
  expect(useAwarenessStore.getState().localClientId).toBe(p.doc.clientID);
});

it("REG-SELF: superseded sessions leave no duplicate self cursors or editors", async () => {
  useAuthStore.setState({ accessToken: "audit-token" });
  const resolves: Array<(v: unknown) => void> = [];
  h.ticket.mockImplementation(() => new Promise((resolve) => resolves.push(resolve)));
  const opts = options({ initialContent: "" });
  // Title re-renders change the callback while ticket requests are pending.
  const hook = renderHook(
    ({ callback }) => useCollabEditor({ ...opts, onTitleUpdate: callback }),
    { initialProps: { callback: () => {} } },
  );
  hook.rerender({ callback: () => {} });
  hook.rerender({ callback: () => {} });
  expect(resolves).toHaveLength(3);
  await act(async () => {
    resolves.forEach((resolve) => resolve({ ticket: "audit-ticket" }));
  });
  await settle();
  // Only the live effect may own a session; superseded setups must be cancelled.
  expect(h.providers).toHaveLength(1);
  const editors = opts.container.querySelectorAll<HTMLElement>(".cm-editor");
  expect(editors).toHaveLength(1);
  const view = EditorView.findFromDOM(editors[0])!;
  expect(view.dom.querySelectorAll(".cm-ySelectionInfo")).toHaveLength(0);
});
