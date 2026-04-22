import { describe, it, expect } from "vitest";
import { PALETTE, assignColor } from "../awarenessColors";

describe("PALETTE", () => {
  it("has exactly 8 colors", () => {
    expect(PALETTE).toHaveLength(8);
  });

  it("all entries are hex color strings", () => {
    for (const c of PALETTE) {
      expect(c).toMatch(/^#[0-9A-Fa-f]{6}$/);
    }
  });

  it("all colors are unique", () => {
    expect(new Set(PALETTE).size).toBe(8);
  });
});

describe("assignColor", () => {
  it("returns first palette color when no colors in use", () => {
    expect(assignColor([])).toBe(PALETTE[0]);
  });

  it("skips colors already in use", () => {
    const color = assignColor([PALETTE[0]]);
    expect(color).toBe(PALETTE[1]);
  });

  it("skips multiple used colors", () => {
    const used = [PALETTE[0], PALETTE[1], PALETTE[2]];
    expect(assignColor(used)).toBe(PALETTE[3]);
  });

  it("wraps around when all colors used (returns first)", () => {
    const color = assignColor([...PALETTE]);
    expect(color).toBe(PALETTE[0]);
  });

  it("handles non-palette colors in used list gracefully", () => {
    const color = assignColor(["#000000"]);
    expect(color).toBe(PALETTE[0]);
  });
});
