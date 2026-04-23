import { describe, it, expect, vi, afterEach } from "vitest";
import {
  loginApi,
  refreshApi,
  forgotPasswordApi,
  resetPasswordApi,
  getMeApi,
  changePasswordApi,
  deleteAccountApi,
  exportAccountDataApi,
} from "../api";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("loginApi", () => {
  it("returns access_token on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ access_token: "jwt_token", token_type: "Bearer" }),
      })
    );

    const result = await loginApi("user@example.com", "password123");
    expect(result.access_token).toBe("jwt_token");
    expect(result.token_type).toBe("Bearer");
  });

  it("throws with server detail on 401", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Invalid email or password" }),
      })
    );

    await expect(loginApi("x@x.com", "wrong")).rejects.toThrow(
      "Invalid email or password"
    );
  });

  it("throws generic message when detail missing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({}),
      })
    );

    await expect(loginApi("x@x.com", "wrong")).rejects.toThrow("Login failed");
  });

  it("sends credentials: include", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ access_token: "t", token_type: "Bearer" }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await loginApi("a@b.com", "pass");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({ credentials: "include" })
    );
  });

  it("returns welcome_doc_id when present in response", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          access_token: "jwt_token",
          token_type: "Bearer",
          welcome_doc_id: "550e8400-e29b-41d4-a716-446655440000",
        }),
      })
    );

    const result = await loginApi("new@example.com", "password123");
    expect(result.welcome_doc_id).toBe("550e8400-e29b-41d4-a716-446655440000");
  });

  it("welcome_doc_id is absent on returning user login", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ access_token: "jwt_token", token_type: "Bearer" }),
      })
    );

    const result = await loginApi("existing@example.com", "password123");
    expect(result.welcome_doc_id).toBeUndefined();
  });
});

describe("refreshApi", () => {
  it("returns new access_token on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          access_token: "new_jwt",
          token_type: "Bearer",
        }),
      })
    );

    const result = await refreshApi();
    expect(result.access_token).toBe("new_jwt");
  });

  it("throws on 401 response", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({}),
      })
    );

    await expect(refreshApi()).rejects.toThrow("Session expired");
  });
});

describe("forgotPasswordApi", () => {
  it("returns message on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          message: "If an account with that email exists, a reset link has been sent.",
        }),
      })
    );

    const result = await forgotPasswordApi("user@example.com");
    expect(result.message).toContain("reset link");
  });

  it("throws with server detail on error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Bad request" }),
      })
    );

    await expect(forgotPasswordApi("x@x.com")).rejects.toThrow("Bad request");
  });

  it("sends POST with JSON body", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ message: "sent" }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await forgotPasswordApi("a@b.com");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({ "Content-Type": "application/json" }),
      })
    );
  });
});

describe("resetPasswordApi", () => {
  it("returns message on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          message: "Password has been reset successfully.",
        }),
      })
    );

    const result = await resetPasswordApi("token123", "newPassword");
    expect(result.message).toContain("reset");
  });

  it("throws on expired token", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Reset token has expired" }),
      })
    );

    await expect(resetPasswordApi("expired", "new")).rejects.toThrow(
      "Reset token has expired"
    );
  });

  it("throws generic when detail missing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({}),
      })
    );

    await expect(resetPasswordApi("bad", "new")).rejects.toThrow(
      "Failed to reset password"
    );
  });
});

describe("getMeApi", () => {
  it("returns profile on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          id: "user-id",
          email: "user@example.com",
          email_verified_at: "2024-01-01T00:00:00Z",
          created_at: "2024-01-01T00:00:00Z",
        }),
      })
    );

    const result = await getMeApi("token");
    expect(result.email).toBe("user@example.com");
  });

  it("throws server detail on failure", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        json: async () => ({ detail: "Unauthorized" }),
      })
    );

    await expect(getMeApi("token")).rejects.toThrow("Unauthorized");
  });
});

describe("changePasswordApi", () => {
  it("posts current and new password", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ message: "Password updated successfully." }),
    });
    vi.stubGlobal("fetch", mockFetch);

    const result = await changePasswordApi("token", "old-pass", "new-pass-123");
    expect(result.message).toContain("updated");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        headers: expect.objectContaining({
          Authorization: "Bearer token",
          "Content-Type": "application/json",
        }),
      })
    );
  });
});

describe("deleteAccountApi", () => {
  it("sends delete request with current password", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ message: "Account deleted successfully." }),
    });
    vi.stubGlobal("fetch", mockFetch);

    await deleteAccountApi("token", "current-pass");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        method: "DELETE",
        body: JSON.stringify({ current_password: "current-pass" }),
      })
    );
  });
});

describe("exportAccountDataApi", () => {
  it("returns export-started message", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ message: "Export started. Check your email." }),
      })
    );

    const result = await exportAccountDataApi("token");
    expect(result.message).toContain("Export started");
  });
});
