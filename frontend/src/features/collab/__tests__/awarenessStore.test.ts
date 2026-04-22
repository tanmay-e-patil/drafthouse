import { describe, it, expect, beforeEach } from "vitest";
import { useAwarenessStore } from "../awarenessStore";

const IDLE_MS = 30_000;
const EXPIRED_MS = 5 * 60_000;

function makePeer(overrides: Partial<ReturnType<typeof useAwarenessStore.getState>["peers"][number]> = {}) {
  return {
    clientId: 1,
    name: "alice",
    color: "#E53E3E",
    lastActive: Date.now(),
    ...overrides,
  };
}

beforeEach(() => {
  useAwarenessStore.setState({ peers: [], localClientId: null });
});

describe("useAwarenessStore", () => {
  it("starts with empty peers", () => {
    expect(useAwarenessStore.getState().peers).toHaveLength(0);
  });

  it("setPeers replaces peers list", () => {
    const peer = makePeer();
    useAwarenessStore.getState().setPeers([peer]);
    expect(useAwarenessStore.getState().peers).toHaveLength(1);
    expect(useAwarenessStore.getState().peers[0].name).toBe("alice");
  });

  it("setPeers with empty array clears peers", () => {
    useAwarenessStore.getState().setPeers([makePeer()]);
    useAwarenessStore.getState().setPeers([]);
    expect(useAwarenessStore.getState().peers).toHaveLength(0);
  });

  it("setLocalClientId stores local client id", () => {
    useAwarenessStore.getState().setLocalClientId(42);
    expect(useAwarenessStore.getState().localClientId).toBe(42);
  });
});

describe("isIdle / isExpired helpers", () => {
  it("peer active recently is not idle", () => {
    const { isIdle } = useAwarenessStore.getState();
    expect(isIdle({ lastActive: Date.now() - 100 })).toBe(false);
  });

  it("peer inactive for 30s+ is idle", () => {
    const { isIdle } = useAwarenessStore.getState();
    expect(isIdle({ lastActive: Date.now() - IDLE_MS - 1 })).toBe(true);
  });

  it("peer inactive for 5min+ is expired", () => {
    const { isExpired } = useAwarenessStore.getState();
    expect(isExpired({ lastActive: Date.now() - EXPIRED_MS - 1 })).toBe(true);
  });

  it("peer inactive for 4min is not expired", () => {
    const { isExpired } = useAwarenessStore.getState();
    expect(isExpired({ lastActive: Date.now() - 4 * 60_000 })).toBe(false);
  });
});
