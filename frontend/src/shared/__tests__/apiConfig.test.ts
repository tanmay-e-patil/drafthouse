import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.resetModules();
});

async function config(dev = false) {
  vi.stubEnv("DEV", dev);
  vi.stubEnv("VITE_API_URL", undefined);
  vi.stubEnv("VITE_WS_URL", undefined);
  return import("../apiConfig");
}

describe("API endpoints", () => {
  it("uses the deployment origin and API prefix for HTTPS WebSockets", async () => {
    vi.stubGlobal("window", { location: { origin: "https://draft.example.com" } });
    const { API_BASE, getWsBase } = await config();
    expect(API_BASE).toBe("/api");
    expect(getWsBase()).toBe("wss://draft.example.com/api");
  });

  it("uses ws on HTTP deployments", async () => {
    vi.stubGlobal("window", { location: { origin: "http://draft.example.com" } });
    const { getWsBase } = await config();
    expect(getWsBase()).toBe("ws://draft.example.com/api");
  });

  it("preserves direct backend access during local development", async () => {
    const { API_BASE, getWsBase } = await config(true);
    expect(API_BASE).toBe("http://localhost:8080");
    expect(getWsBase()).toBe("ws://localhost:8080");
  });

  it("honors explicit API and WebSocket overrides", async () => {
    await config();
    vi.resetModules();
    vi.stubEnv("VITE_API_URL", "https://api.example.com");
    vi.stubEnv("VITE_WS_URL", "wss://ws.example.com");
    const { API_BASE, getWsBase } = await import("../apiConfig");
    expect(API_BASE).toBe("https://api.example.com");
    expect(getWsBase()).toBe("wss://ws.example.com");
  });

  it("can be imported during SSR without window", async () => {
    vi.stubGlobal("window", undefined);
    const { API_BASE } = await config();
    expect(API_BASE).toBe("/api");
  });
});
