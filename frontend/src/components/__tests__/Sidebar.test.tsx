import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import Sidebar from "../Sidebar";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  useParams: () => ({}),
}));

beforeEach(() => {
  useDocumentStore.getState().reset();
  useAuthStore.getState().setAccessToken("test_token");
  vi.restoreAllMocks();
});

describe("Sidebar", () => {
  it("renders empty state when no documents", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ data: [], next_cursor: null, has_more: false }),
      })
    );

    render(<Sidebar />);

    await waitFor(() => {
      expect(screen.getByText("No documents yet")).toBeDefined();
    });
    expect(
      screen.getByText("Create your first document")
    ).toBeDefined();
  });

  it("renders document list when documents exist", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          data: [
            {
              id: "1",
              owner_id: "owner",
              title: "Test Document",
              is_public: false,
              created_at: "2024-01-01T00:00:00Z",
              updated_at: "2024-01-01T00:00:00Z",
            },
          ],
          next_cursor: null,
          has_more: false,
        }),
      })
    );

    render(<Sidebar />);

    await waitFor(() => {
      expect(screen.getByText("Test Document")).toBeDefined();
    });
  });

  it("renders load more button when hasMore is true", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          data: [
            {
              id: "1",
              owner_id: "owner",
              title: "Doc 1",
              is_public: false,
              created_at: "2024-01-01T00:00:00Z",
              updated_at: "2024-01-01T00:00:00Z",
            },
          ],
          next_cursor: "cursor123",
          has_more: true,
        }),
      })
    );

    render(<Sidebar />);

    await waitFor(() => {
      expect(screen.getByText("Load more")).toBeDefined();
    });
  });

  it("does not render load more button when no more pages", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          data: [
            {
              id: "1",
              owner_id: "owner",
              title: "Doc 1",
              is_public: false,
              created_at: "2024-01-01T00:00:00Z",
              updated_at: "2024-01-01T00:00:00Z",
            },
          ],
          next_cursor: null,
          has_more: false,
        }),
      })
    );

    render(<Sidebar />);

    await waitFor(() => {
      expect(screen.getByText("Doc 1")).toBeDefined();
    });
    expect(screen.queryByText("Load more")).toBeNull();
  });

  it("renders sidebar header with title and new button", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          data: [
            {
              id: "1",
              owner_id: "owner",
              title: "Doc",
              is_public: false,
              created_at: "2024-01-01T00:00:00Z",
              updated_at: "2024-01-01T00:00:00Z",
            },
          ],
          next_cursor: null,
          has_more: false,
        }),
      })
    );

    render(<Sidebar />);

    await waitFor(() => {
      expect(screen.getByText("Documents")).toBeDefined();
      expect(screen.getByText("+ New")).toBeDefined();
    });
  });
});
