import { beforeEach, describe, expect, it, vi } from "vitest";

describe("preferences store", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  it("persists the selected editor font across store creation", async () => {
    const { usePreferencesStore } = await import("../store");

    usePreferencesStore.getState().setEditorFont("lora");

    vi.resetModules();
    const reloaded = await import("../store");
    expect(reloaded.usePreferencesStore.getState().editorFont).toBe("lora");
  });

  it("keeps focus mode as session-only state", async () => {
    const { usePreferencesStore } = await import("../store");

    usePreferencesStore.getState().setFocusMode(true);

    expect(usePreferencesStore.getState().focusMode).toBe(true);
    expect(localStorage.getItem("dh_focus_mode")).toBeNull();
  });
});
