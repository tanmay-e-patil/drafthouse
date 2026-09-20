/**
 * Regression test for the auth audit (docs/BUG_AUDIT.md #17).
 *
 * Requires: a 401 from an authenticated API call triggers a single token
 * refresh, then retries the original request with the new access token.
 */
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useAuthStore } from "#/features/auth/store";
import { updateDocumentContentApi } from "../api";

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  useAuthStore.setState({ accessToken: "expired-token", hydrated: true });
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("REG-17: a 401 save triggers one refresh and retries with the new token", async () => {
  fetchMock
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ detail: "token expired" }), { status: 401 }),
    )
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ access_token: "refreshed-token" }), { status: 200 }),
    )
    .mockResolvedValueOnce(new Response(null, { status: 200 }));

  await expect(updateDocumentContentApi("audit-doc", "new text")).resolves.toBeUndefined();

  expect(fetchMock).toHaveBeenCalledTimes(3);
  expect(fetchMock.mock.calls[0][0]).toContain("/documents/audit-doc/content");
  expect(fetchMock.mock.calls[1][0]).toContain("/auth/refresh");
  expect(fetchMock.mock.calls[2][0]).toContain("/documents/audit-doc/content");
  expect(fetchMock.mock.calls[2][1].headers.Authorization).toBe("Bearer refreshed-token");
});

it("REG-17: concurrent 401s share a single refresh (single-flight)", async () => {
  useAuthStore.setState({ accessToken: "expired-token", hydrated: true });
  fetchMock
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ detail: "token expired" }), { status: 401 }),
    )
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ detail: "token expired" }), { status: 401 }),
    )
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ access_token: "refreshed-token" }), { status: 200 }),
    )
    .mockResolvedValueOnce(new Response(null, { status: 200 }))
    .mockResolvedValueOnce(new Response(null, { status: 200 }));

  const results = await Promise.allSettled([
    updateDocumentContentApi("audit-doc", "x"),
    updateDocumentContentApi("audit-doc", "y"),
  ]);

  expect(results.every((r) => r.status === "fulfilled")).toBe(true);
  const refreshCalls = fetchMock.mock.calls.filter((c: any[]) =>
    String(c[0]).includes("/auth/refresh"),
  );
  expect(refreshCalls).toHaveLength(1);
  expect(fetchMock).toHaveBeenCalledTimes(5);
});

it("REG-17: a failure after refresh surfaces the error instead of looping", async () => {
  fetchMock
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ detail: "token expired" }), { status: 401 }),
    )
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ access_token: "refreshed-token" }), { status: 200 }),
    )
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ detail: "still forbidden" }), { status: 403 }),
    );

  await expect(updateDocumentContentApi("audit-doc", "new text")).rejects.toThrow(
    "still forbidden",
  );
  expect(fetchMock).toHaveBeenCalledTimes(3);
});
