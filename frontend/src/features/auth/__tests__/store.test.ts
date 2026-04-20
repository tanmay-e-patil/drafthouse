import { describe, it, expect, beforeEach, vi } from "vitest";
import { useAuthStore } from "../store";

vi.mock("../api", () => ({
  refreshApi: vi.fn(),
}));

describe("useAuthStore", () => {
  beforeEach(() => {
    useAuthStore.setState({ accessToken: null, hydrated: false });
    vi.clearAllMocks();
  });

  it("starts with no access token and not hydrated", () => {
    const state = useAuthStore.getState();
    expect(state.accessToken).toBeNull();
    expect(state.hydrated).toBe(false);
  });

  it("setAccessToken stores the token", () => {
    useAuthStore.getState().setAccessToken("tok_abc123");
    expect(useAuthStore.getState().accessToken).toBe("tok_abc123");
  });

  it("clearAuth removes the token", () => {
    useAuthStore.getState().setAccessToken("tok_xyz");
    useAuthStore.getState().clearAuth();
    expect(useAuthStore.getState().accessToken).toBeNull();
  });

  it("hydrate sets token on successful refresh", async () => {
    const { refreshApi } = await import("../api");
    (refreshApi as ReturnType<typeof vi.fn>).mockResolvedValue({
      access_token: "refreshed_tok",
      token_type: "Bearer",
    });

    await useAuthStore.getState().hydrate();

    expect(useAuthStore.getState().accessToken).toBe("refreshed_tok");
    expect(useAuthStore.getState().hydrated).toBe(true);
  });

  it("hydrate stays null on failed refresh", async () => {
    const { refreshApi } = await import("../api");
    (refreshApi as ReturnType<typeof vi.fn>).mockRejectedValue(new Error("expired"));

    await useAuthStore.getState().hydrate();

    expect(useAuthStore.getState().accessToken).toBeNull();
    expect(useAuthStore.getState().hydrated).toBe(true);
  });
});
