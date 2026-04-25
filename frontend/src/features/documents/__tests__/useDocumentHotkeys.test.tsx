import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { useDocumentHotkeys } from "../useDocumentHotkeys";

function TestHarness(props: {
  onOpenPalette: () => void;
  onToggleSidebar: () => void;
  onToggleFocusMode?: () => void;
}) {
  useDocumentHotkeys(props);
  return null;
}

describe("useDocumentHotkeys", () => {
  it("opens the command palette on Cmd/Ctrl+K", () => {
    const onOpenPalette = vi.fn();
    const onToggleSidebar = vi.fn();

    render(
      <TestHarness
        onOpenPalette={onOpenPalette}
        onToggleSidebar={onToggleSidebar}
      />,
    );

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true }));

    expect(onOpenPalette).toHaveBeenCalledTimes(1);
    expect(onToggleSidebar).not.toHaveBeenCalled();
  });

  it("toggles the sidebar on Cmd/Ctrl+Shift+Backslash", () => {
    const onOpenPalette = vi.fn();
    const onToggleSidebar = vi.fn();

    render(
      <TestHarness
        onOpenPalette={onOpenPalette}
        onToggleSidebar={onToggleSidebar}
      />,
    );

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "\\", metaKey: true, shiftKey: true }));

    expect(onToggleSidebar).toHaveBeenCalledTimes(1);
    expect(onOpenPalette).not.toHaveBeenCalled();
  });

  it("toggles focus mode on Cmd/Ctrl+Shift+Period", () => {
    const onOpenPalette = vi.fn();
    const onToggleSidebar = vi.fn();
    const onToggleFocusMode = vi.fn();

    render(
      <TestHarness
        onOpenPalette={onOpenPalette}
        onToggleSidebar={onToggleSidebar}
        onToggleFocusMode={onToggleFocusMode}
      />,
    );

    window.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: ">",
        code: "Period",
        metaKey: true,
        shiftKey: true,
      }),
    );

    expect(onToggleFocusMode).toHaveBeenCalledTimes(1);
    expect(onOpenPalette).not.toHaveBeenCalled();
    expect(onToggleSidebar).not.toHaveBeenCalled();
  });

  it("does not use Cmd/Ctrl+Shift+F because browsers reserve it", () => {
    const onOpenPalette = vi.fn();
    const onToggleSidebar = vi.fn();
    const onToggleFocusMode = vi.fn();

    render(
      <TestHarness
        onOpenPalette={onOpenPalette}
        onToggleSidebar={onToggleSidebar}
        onToggleFocusMode={onToggleFocusMode}
      />,
    );

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "F", metaKey: true, shiftKey: true }),
    );

    expect(onToggleFocusMode).not.toHaveBeenCalled();
  });

});
