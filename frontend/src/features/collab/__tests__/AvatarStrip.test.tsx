import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useAwarenessStore } from "../awarenessStore";
import AvatarStrip from "../ui/AvatarStrip";

function makePeer(id: number, name: string, lastActive = Date.now()) {
  return { clientId: id, name, color: "#E53E3E", lastActive };
}

beforeEach(() => {
  useAwarenessStore.setState({ peers: [], localClientId: null });
});

describe("AvatarStrip", () => {
  it("renders nothing when no peers", () => {
    const { container } = render(<AvatarStrip />);
    expect(container.firstChild).toBeNull();
  });

  it("renders one avatar per peer", () => {
    useAwarenessStore.setState({
      peers: [makePeer(1, "alice"), makePeer(2, "bob")],
      localClientId: 99,
    });
    render(<AvatarStrip />);
    expect(screen.getByTitle("alice")).toBeDefined();
    expect(screen.getByTitle("bob")).toBeDefined();
  });

  it("excludes own peer (localClientId)", () => {
    useAwarenessStore.setState({
      peers: [makePeer(1, "alice"), makePeer(99, "me")],
      localClientId: 99,
    });
    render(<AvatarStrip />);
    expect(screen.queryByTitle(/me/i)).toBeNull();
    expect(screen.getByTitle(/alice/i)).toBeDefined();
  });

  it("shows max 5 avatars + overflow badge for 6+ peers", () => {
    const peers = Array.from({ length: 7 }, (_, i) => makePeer(i + 1, `user${i + 1}`));
    useAwarenessStore.setState({ peers, localClientId: 99 });
    render(<AvatarStrip />);
    expect(screen.getByText("+2")).toBeDefined();
  });

  it("shows no overflow badge for 5 or fewer peers", () => {
    const peers = Array.from({ length: 5 }, (_, i) => makePeer(i + 1, `user${i + 1}`));
    useAwarenessStore.setState({ peers, localClientId: 99 });
    render(<AvatarStrip />);
    expect(screen.queryByText(/^\+\d/)).toBeNull();
  });

  it("excludes expired peers (5min idle)", () => {
    const expiredActive = Date.now() - 6 * 60_000;
    useAwarenessStore.setState({
      peers: [makePeer(1, "alice"), makePeer(2, "ghost", expiredActive)],
      localClientId: 99,
    });
    render(<AvatarStrip />);
    expect(screen.getByTitle(/alice/i)).toBeDefined();
    expect(screen.queryByTitle(/ghost/i)).toBeNull();
  });

  it("shows idle peers grayed out (30s idle)", () => {
    const idleActive = Date.now() - 35_000;
    useAwarenessStore.setState({
      peers: [makePeer(1, "alice", idleActive)],
      localClientId: 99,
    });
    render(<AvatarStrip />);
    const avatar = screen.getByTitle(/alice/i);
    expect(avatar.style.opacity).toBe("0.35");
  });
});
