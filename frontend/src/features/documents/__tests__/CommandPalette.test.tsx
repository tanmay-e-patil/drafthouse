import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CommandPalette } from "../CommandPalette";
import { useDocumentStore } from "../store";
import { useAuthStore } from "#/features/auth/store";

const navigate = vi.fn();

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigate,
}));

describe("CommandPalette", () => {
  beforeEach(() => {
    navigate.mockReset();
    useDocumentStore.getState().reset();
    useAuthStore.setState({ accessToken: "token", hydrated: true });
    vi.restoreAllMocks();
  });

  it("filters documents by title and navigates on Enter", async () => {
    useDocumentStore.setState({
      documents: [
        {
          id: "doc-1",
          owner_id: "owner",
          title: "Alpha notes",
          is_public: false,
          created_at: "2024-01-01T00:00:00Z",
          updated_at: "2024-01-03T00:00:00Z",
        },
        {
          id: "doc-2",
          owner_id: "owner",
          title: "Beta outline",
          is_public: false,
          created_at: "2024-01-01T00:00:00Z",
          updated_at: "2024-01-02T00:00:00Z",
        },
      ],
      hasMore: false,
      nextCursor: null,
    });

    render(<CommandPalette open onOpenChange={vi.fn()} />);

    const input = await screen.findByLabelText("Search documents");
    fireEvent.change(input, { target: { value: "beta" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(navigate).toHaveBeenCalledWith({
        to: "/documents/$documentId",
        params: { documentId: "doc-2" },
      });
    });
  });

  it("loads additional pages when the store is incomplete", async () => {
    useDocumentStore.setState({
      documents: [
        {
          id: "doc-1",
          owner_id: "owner",
          title: "Alpha notes",
          is_public: false,
          created_at: "2024-01-01T00:00:00Z",
          updated_at: "2024-01-03T00:00:00Z",
        },
      ],
      hasMore: true,
      nextCursor: "cursor-1",
    });

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          data: [
            {
              id: "doc-2",
              owner_id: "owner",
              title: "Beta outline",
              is_public: false,
              created_at: "2024-01-01T00:00:00Z",
              updated_at: "2024-01-02T00:00:00Z",
            },
          ],
          next_cursor: null,
          has_more: false,
        }),
      }),
    );

    render(<CommandPalette open onOpenChange={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Beta outline")).toBeDefined();
    });

    expect(useDocumentStore.getState().documents).toHaveLength(2);
  });
});
