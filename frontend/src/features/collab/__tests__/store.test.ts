import { describe, it, expect, beforeEach } from "vitest";
import { useCollabStore } from "../store";

beforeEach(() => {
  useCollabStore.setState({ status: "disconnected" });
});

describe("useCollabStore", () => {
  it("starts disconnected", () => {
    expect(useCollabStore.getState().status).toBe("disconnected");
  });

  it("setStatus updates status", () => {
    useCollabStore.getState().setStatus("connected");
    expect(useCollabStore.getState().status).toBe("connected");
  });

  it("setStatus transitions through all states", () => {
    const { setStatus } = useCollabStore.getState();
    setStatus("connecting");
    expect(useCollabStore.getState().status).toBe("connecting");
    setStatus("syncing");
    expect(useCollabStore.getState().status).toBe("syncing");
    setStatus("connected");
    expect(useCollabStore.getState().status).toBe("connected");
    setStatus("disconnected");
    expect(useCollabStore.getState().status).toBe("disconnected");
  });
});
