import { describe, it, expect, vi, afterEach } from "vitest";
import { loginApi, refreshApi } from "../api";

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
