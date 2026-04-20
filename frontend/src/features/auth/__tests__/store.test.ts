import { describe, it, expect, beforeEach } from "vitest";
import { useAuthStore } from "../store";

describe("useAuthStore", () => {
  beforeEach(() => {
    useAuthStore.setState({ accessToken: null });
  });

  it("starts with no access token", () => {
    expect(useAuthStore.getState().accessToken).toBeNull();
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
});
