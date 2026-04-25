import { useEffect } from "react";

interface UseDocumentHotkeysOptions {
  onOpenPalette: () => void;
  onToggleSidebar: () => void;
  onToggleFocusMode?: () => void;
}

export function useDocumentHotkeys({
  onOpenPalette,
  onToggleSidebar,
  onToggleFocusMode,
}: UseDocumentHotkeysOptions) {
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const modifierPressed = event.metaKey || event.ctrlKey;
      if (!modifierPressed) return;

      if (!event.shiftKey && !event.altKey && event.key.toLowerCase() === "k") {
        event.preventDefault();
        onOpenPalette();
        return;
      }

      if (event.shiftKey && event.key === "\\") {
        event.preventDefault();
        onToggleSidebar();
        return;
      }

      const isPeriodKey = event.code === "Period" || event.key === "." || event.key === ">";
      if (event.shiftKey && !event.altKey && isPeriodKey) {
        event.preventDefault();
        onToggleFocusMode?.();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onOpenPalette, onToggleFocusMode, onToggleSidebar]);
}
