/**
 * Regression tests for the document route audit
 * (docs/BUG_AUDIT.md #12, #16, #18).
 *
 * The real document route component is rendered with controlled router
 * parameters and API responses; child UI is stubbed. These tests pin the
 * route's state ownership and the props handed to the collaboration editor.
 */
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { useAuthStore } from "#/features/auth/store";

const h = vi.hoisted(() => ({
  id: "A",
  props: null as any,
  get: vi.fn(),
  content: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@tanstack/react-router", () => ({
  createFileRoute: () => (options: any) => options,
  useParams: () => ({ documentId: h.id }),
  Link: () => null,
}));
vi.mock("#/features/documents/api", () => ({
  getDocumentApi: h.get,
  getDocumentContentApi: h.content,
  updateDocumentApi: h.update,
}));
vi.mock("#/widgets/editor/Editor", () => ({
  default: (props: any) => {
    h.props = props;
    useEffect(
      () => () => {
        if (h.props === props) h.props = null;
      },
      [props],
    );
    return <div>Audit editor</div>;
  },
}));
vi.mock("#/components/Sidebar", () => ({ default: () => null }));
vi.mock("#/features/documents/CommandPalette", () => ({ CommandPalette: () => null }));
vi.mock("#/features/documents/ShareModal", () => ({ ShareModal: () => null }));
vi.mock("#/features/documents/useDocumentHotkeys", () => ({ useDocumentHotkeys: () => {} }));
vi.mock("#/routes/-documentStates", () => ({
  DocumentLoadingState: () => <div>Loading</div>,
  InaccessibleDocumentState: () => <div>Inaccessible</div>,
}));
vi.mock("#/shared/errors", async (original) => ({
  ...(await original<object>()),
  notifyTransientError: vi.fn(),
}));

import { Route } from "#/routes/documents.$documentId";

const Component = (Route as any).component;
const doc = (id: string, role = "owner") => ({
  id,
  owner_id: "owner",
  title: id,
  access_role: role,
  is_public: false,
  created_at: "",
  updated_at: "",
});

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

beforeEach(() => {
  h.id = "A";
  h.props = null;
  h.get.mockReset();
  h.content.mockReset();
  h.update.mockReset();
  useAuthStore.setState({ accessToken: "token", hydrated: true, hydrate: vi.fn(async () => {}) });
});

afterEach(cleanup);

it("REG-18: a late response for document A never overwrites document B state", async () => {
  const aDoc = deferred<any>();
  const aContent = deferred<any>();
  h.get.mockImplementation((id: string) => (id === "A" ? aDoc.promise : Promise.resolve(doc("B"))));
  h.content.mockImplementation((id: string) =>
    id === "A" ? aContent.promise : Promise.resolve({ content: "B text" }),
  );
  const ui = render(<Component />);
  h.id = "B";
  ui.rerender(<Component />);
  await waitFor(() => expect(h.props?.initialContent).toBe("B text"));
  await act(async () => {
    aDoc.resolve(doc("A"));
    aContent.resolve({ content: "A text" });
  });
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  expect(h.props.docId).toBe("B");
  expect(h.props.initialContent).toBe("B text");
});

it("REG-04: the editor receives no plaintext save callback", async () => {
  h.get.mockResolvedValue(doc("A"));
  h.content.mockResolvedValue({ content: "A text" });
  render(<Component />);
  await waitFor(() => expect(h.props?.docId).toBe("A"));
  expect(h.props.onSave).toBeUndefined();
});

it("REG-18: a failed load for B never serves A's content under B", async () => {
  h.get.mockResolvedValue(doc("A"));
  h.content.mockResolvedValue({ content: "A text" });
  const ui = render(<Component />);
  await waitFor(() => expect(h.props?.docId).toBe("A"));
  h.get.mockRejectedValue(new Error("temporary failure"));
  h.id = "B";
  ui.rerender(<Component />);
  await waitFor(() => expect(ui.queryByText("Loading")).toBeNull());
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  expect(h.props).toBeNull();
});

it("REG-16: editor members are not offered a title edit the API rejects", async () => {
  h.get.mockResolvedValue(doc("A", "editor"));
  h.content.mockResolvedValue({ content: "A text" });
  const ui = render(<Component />);
  const title = await ui.findByLabelText("Document title");
  expect((title as HTMLInputElement).disabled).toBe(true);
});

it("REG-12: typing a title keeps the collaboration callback stable", async () => {
  h.get.mockResolvedValue(doc("A"));
  h.content.mockResolvedValue({ content: "A text" });
  const ui = render(<Component />);
  const title = await ui.findByLabelText("Document title");
  const previous = h.props.onTitleUpdate;
  fireEvent.change(title, { target: { value: "typing" } });
  expect(h.props.onTitleUpdate).toBe(previous);
});
