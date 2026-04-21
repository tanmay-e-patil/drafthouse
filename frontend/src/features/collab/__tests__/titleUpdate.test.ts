import { describe, it, expect } from "vitest";
import { decodeTitleUpdate, encodeTitleUpdate } from "../titleUpdate";

describe("decodeTitleUpdate", () => {
  it("decodes a simple title", () => {
    const encoded = encodeTitleUpdate("Hello World");
    const result = decodeTitleUpdate(encoded);
    expect(result).toBe("Hello World");
  });

  it("decodes an empty title", () => {
    const encoded = encodeTitleUpdate("");
    expect(decodeTitleUpdate(encoded)).toBe("");
  });

  it("decodes unicode title", () => {
    const encoded = encodeTitleUpdate("日本語タイトル");
    expect(decodeTitleUpdate(encoded)).toBe("日本語タイトル");
  });

  it("returns null for non-type-3 message", () => {
    const notType3 = new Uint8Array([0, 1, 2]);
    expect(decodeTitleUpdate(notType3)).toBeNull();
  });

  it("returns null for empty buffer", () => {
    expect(decodeTitleUpdate(new Uint8Array([]))).toBeNull();
  });

  it("returns null when buffer is too short for declared length", () => {
    const truncated = new Uint8Array([3, 10, 65]); // claims len=10 but only 1 byte after
    expect(decodeTitleUpdate(truncated)).toBeNull();
  });

  it("roundtrip: encode then decode returns original title", () => {
    const titles = ["Untitled", "My Research Notes", "Draft #3 — Final", ""];
    for (const title of titles) {
      expect(decodeTitleUpdate(encodeTitleUpdate(title))).toBe(title);
    }
  });
});
