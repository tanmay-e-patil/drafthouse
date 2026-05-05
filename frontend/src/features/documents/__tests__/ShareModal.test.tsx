import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ShareModal } from "../ShareModal";
import * as api from "../api";

vi.mock("../api", () => ({
  listMembersApi: vi.fn().mockResolvedValue([]),
  listInviteLinksApi: vi.fn().mockResolvedValue([]),
  createInviteLinkApi: vi.fn(),
  revokeInviteLinkApi: vi.fn(),
  removeMemberApi: vi.fn(),
  updateMemberRoleApi: vi.fn(),
  updateDocumentApi: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const defaultProps = {
  docId: "doc-123",
  docTitle: "My Doc",
  isPublic: false,
  onClose: vi.fn(),
  onPublicToggle: vi.fn(),
};

beforeEach(() => {
  vi.mocked(api.listMembersApi).mockResolvedValue([]);
  vi.mocked(api.listInviteLinksApi).mockResolvedValue([]);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("ShareModal", () => {
  it("renders title with document name", async () => {
    render(<ShareModal {...defaultProps} />);
    expect(screen.getByText(/Share "My Doc"/)).toBeTruthy();
  });

  it("shows empty state when no members", async () => {
    render(<ShareModal {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("No members yet")).toBeTruthy();
    });
  });

  it("shows members when loaded", async () => {
    vi.mocked(api.listMembersApi).mockResolvedValue([
      {
        doc_id: "doc-123",
        user_id: "user-abc-1234",
        email: "friend@example.com",
        role: "editor",
      },
    ]);
    render(<ShareModal {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("friend@example.com")).toBeTruthy();
    });
  });

  it("shows one labeled invite slot per role", async () => {
    vi.mocked(api.listInviteLinksApi).mockResolvedValue([
      {
        token: "editortoken123",
        doc_id: "doc-123",
        role: "editor",
        created_by: "owner-id",
        max_uses: null,
        use_count: 0,
        expires_at: null,
        revoked_at: null,
      },
      {
        token: "viewertoken123",
        doc_id: "doc-123",
        role: "viewer",
        created_by: "owner-id",
        max_uses: null,
        use_count: 0,
        expires_at: null,
        revoked_at: null,
      },
    ]);

    render(<ShareModal {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("Editor link")).toBeTruthy();
      expect(screen.getByText("Viewer link")).toBeTruthy();
      expect(screen.getByText(/editortoken123/)).toBeTruthy();
      expect(screen.getByText(/viewertoken123/)).toBeTruthy();
    });
  });

  it("calls createInviteLinkApi for a missing role", async () => {
    vi.mocked(api.createInviteLinkApi).mockResolvedValue({
      token: "newtoken123",
      doc_id: "doc-123",
      role: "editor",
      created_by: "owner-id",
      max_uses: null,
      use_count: 0,
      expires_at: null,
      revoked_at: null,
    });
    render(<ShareModal {...defaultProps} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Generate editor link" }),
    );
    await waitFor(() => {
      expect(api.createInviteLinkApi).toHaveBeenCalledWith("doc-123", {
        role: "editor",
      });
    });
  });

  it("does not create a second invite link for an existing role", async () => {
    const writeText = vi.fn();
    Object.assign(navigator, { clipboard: { writeText } });
    vi.mocked(api.listInviteLinksApi).mockResolvedValue([
      {
        token: "editortoken123",
        doc_id: "doc-123",
        role: "editor",
        created_by: "owner-id",
        max_uses: null,
        use_count: 0,
        expires_at: null,
        revoked_at: null,
      },
    ]);

    render(<ShareModal {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText(/editortoken123/)).toBeTruthy();
    });

    expect(
      screen.queryByRole("button", { name: "Generate editor link" }),
    ).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Copy editor link" }));

    expect(api.createInviteLinkApi).not.toHaveBeenCalled();
    expect(writeText).toHaveBeenCalledWith(
      "http://localhost:3000/invite/editortoken123",
    );
  });

  it("calls onClose when close button clicked", async () => {
    render(<ShareModal {...defaultProps} />);
    const closeBtn = screen.getByRole("button", { name: "Close" });
    fireEvent.click(closeBtn);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });

  it("calls updateDocumentApi on public toggle", async () => {
    vi.mocked(api.updateDocumentApi).mockResolvedValue({
      id: "doc-123",
      owner_id: "owner-id",
      title: "My Doc",
      is_public: true,
      created_at: "",
      updated_at: "",
    });
    render(<ShareModal {...defaultProps} />);
    fireEvent.click(screen.getByRole("switch"));
    await waitFor(() => {
      expect(api.updateDocumentApi).toHaveBeenCalledWith("doc-123", {
        is_public: true,
      });
    });
  });
});
