import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SidebarPresence from "../SidebarPresence";

function peer(name: string, lastActive = Date.now()) {
  return {
    user_id: null,
    name,
    color: "#E53E3E",
    last_active: new Date(lastActive).toISOString(),
  };
}

describe("SidebarPresence", () => {
  it("renders nothing when no other peers are present", () => {
    const { container } = render(
      <SidebarPresence peers={[peer("alice")]} currentUserEmail="alice@example.com" />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders max 3 avatars with overflow", () => {
    render(
      <SidebarPresence
        peers={[peer("a"), peer("b"), peer("c"), peer("d"), peer("e")]}
        currentUserEmail="me@example.com"
      />
    );
    expect(screen.getByText("+2")).toBeDefined();
  });

  it("dims idle peers", () => {
    render(
      <SidebarPresence
        peers={[peer("alice", Date.now() - 35_000)]}
        currentUserEmail="me@example.com"
      />
    );
    expect(screen.getByTitle("alice").style.opacity).toBe("0.35");
  });

  it("omits expired peers", () => {
    render(
      <SidebarPresence
        peers={[peer("ghost", Date.now() - 6 * 60_000), peer("alice")]}
        currentUserEmail="me@example.com"
      />
    );
    expect(screen.queryByTitle("ghost")).toBeNull();
    expect(screen.getByTitle("alice")).toBeDefined();
  });
});
