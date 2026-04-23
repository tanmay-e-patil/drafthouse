import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Sidebar from "../Sidebar";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";

const navigate = vi.fn();

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigate,
  useParams: () => ({}),
}));

vi.mock("next-themes", () => ({
  useTheme: () => ({ theme: "light", setTheme: vi.fn(), resolvedTheme: "light" }),
}));

const noop = () => {};

beforeEach(() => {
  useDocumentStore.getState().reset();
  useAuthStore.setState({ accessToken: "test_token", hydrated: true, email: "test@example.com" });
  vi.restoreAllMocks();
  navigate.mockReset();
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

    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

    await waitFor(() => {
      expect(screen.getByText("No documents yet")).toBeDefined();
    });
  });

  it("renders document list when documents exist", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce({
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
        .mockResolvedValueOnce({
          ok: true,
          json: async () => ({
            data: [],
          }),
        })
    );

    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

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

    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

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

    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

    await waitFor(() => {
      expect(screen.getByText("Doc 1")).toBeDefined();
    });
    expect(screen.queryByText("Load more")).toBeNull();
  });

  it("renders sidebar header with title", async () => {
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

    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

    await waitFor(() => {
      expect(screen.getByText("Drafthouse")).toBeDefined();
      expect(screen.getByText("New document")).toBeDefined();
    });
  });

  it("renders sidebar presence avatars returned by the presence poll", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce({
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
        .mockResolvedValueOnce({
          ok: true,
          json: async () => ({
            data: [
              {
                name: "alice",
                color: "#E53E3E",
                last_active: new Date().toISOString(),
              },
            ],
          }),
        })
    );

    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

    await waitFor(() => {
      expect(screen.getByTitle("alice")).toBeDefined();
    });
  });

  it("shows settings in the avatar menu and navigates to the settings route", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ data: [], next_cursor: null, has_more: false }),
      })
    );

    const user = userEvent.setup();
    render(<Sidebar collapsed={false} onToggleCollapse={noop} />);

    await user.click(screen.getByRole("button", { name: /test/i }));
    await user.click(await screen.findByText("Settings"));

    expect(navigate).toHaveBeenCalledWith({ to: "/settings" });
  });
});
