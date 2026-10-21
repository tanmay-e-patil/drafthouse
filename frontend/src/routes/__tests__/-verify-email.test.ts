import { afterEach, expect, it, vi } from "vitest";

const route = vi.hoisted(() => ({
  loader: (_context: { location: { search: Record<string, string> } }): Promise<unknown> =>
    Promise.resolve(undefined),
}));

vi.mock("@tanstack/react-router", () => ({
  createFileRoute: () => (options: typeof route) => {
    route.loader = options.loader;
    return {};
  },
  Link: () => null,
  useNavigate: vi.fn(),
}));

vi.mock("@tanstack/react-start", () => ({
  createServerFn: () => ({
    validator: () => ({
      handler: (handler: (context: { data: string }) => Promise<unknown>) => handler,
    }),
  }),
}));

vi.mock("@tanstack/react-start/server", () => ({
  getRequestUrl: () => new URL("https://draft.example.com/verify-email?token=token123"),
}));

vi.mock("#/shared/apiConfig", () => ({ API_BASE: "/api" }));

afterEach(() => vi.unstubAllGlobals());

it("resolves the production API path against the incoming verification request", async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({}) });
  vi.stubGlobal("fetch", fetch);
  await import("../verify-email");

  await expect(route.loader({ location: { search: { token: "token123" } } }))
    .resolves.toEqual({ success: true });
  expect(fetch).toHaveBeenCalledWith(
    new URL("https://draft.example.com/api/auth/verify-email"),
    expect.objectContaining({ method: "POST", body: JSON.stringify({ token: "token123" }) }),
  );
});
