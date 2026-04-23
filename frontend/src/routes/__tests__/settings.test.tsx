import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useAuthStore } from "#/features/auth/store";

const {
  navigate,
  toastSuccess,
  toastError,
  getMeApi,
  changePasswordApi,
  exportAccountDataApi,
  deleteAccountApi,
} = vi.hoisted(() => ({
  navigate: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  getMeApi: vi.fn(),
  changePasswordApi: vi.fn(),
  exportAccountDataApi: vi.fn(),
  deleteAccountApi: vi.fn(),
}));

vi.mock("@tanstack/react-router", () => ({
  createFileRoute: () => () => ({}),
  useNavigate: () => navigate,
}));

vi.mock("sonner", () => ({
  toast: {
    success: toastSuccess,
    error: toastError,
  },
}));

vi.mock("#/features/auth/api", () => ({
  getMeApi: (...args: unknown[]) => getMeApi(...args),
  changePasswordApi: (...args: unknown[]) => changePasswordApi(...args),
  exportAccountDataApi: (...args: unknown[]) => exportAccountDataApi(...args),
  deleteAccountApi: (...args: unknown[]) => deleteAccountApi(...args),
}));

vi.mock("#/components/Sidebar", () => ({
  default: () => <div>Sidebar</div>,
}));

const { SettingsPage } = await import("../settings");

describe("SettingsPage", () => {
  beforeEach(() => {
    navigate.mockReset();
    toastSuccess.mockReset();
    toastError.mockReset();
    getMeApi.mockReset();
    changePasswordApi.mockReset();
    exportAccountDataApi.mockReset();
    deleteAccountApi.mockReset();
    useAuthStore.setState({
      accessToken: "test-token",
      hydrated: true,
      email: "test@example.com",
      hydrate: vi.fn(async () => {}),
    });
    getMeApi.mockResolvedValue({
      id: "user-id",
      email: "owner@example.com",
      email_verified_at: "2024-01-01T00:00:00Z",
      created_at: "2024-01-01T00:00:00Z",
    });
  });

  it("renders current email from the profile API", async () => {
    render(<SettingsPage />);
    expect(await screen.findByDisplayValue("owner@example.com")).toBeDefined();
  });

  it("shows client validation errors for password changes", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    await user.click(await screen.findByRole("button", { name: "Update password" }));
    expect(await screen.findByText("All password fields are required")).toBeDefined();
  });

  it("starts export and shows a success toast", async () => {
    const user = userEvent.setup();
    exportAccountDataApi.mockResolvedValue({ message: "Export started. Check your email." });
    render(<SettingsPage />);

    await user.click(await screen.findByRole("button", { name: "Export all documents" }));

    await waitFor(() => {
      expect(exportAccountDataApi).toHaveBeenCalledWith("test-token");
      expect(toastSuccess).toHaveBeenCalledWith("Export started. Check your email.");
    });
  });

  it("requires a password before account deletion", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    await user.click(await screen.findByRole("button", { name: "Delete account" }));
    await user.click(await screen.findByRole("button", { name: /^Delete account$/ }));

    expect(await screen.findByText("Current password is required")).toBeDefined();
  });
});
