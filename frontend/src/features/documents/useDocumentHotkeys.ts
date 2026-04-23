import { useEffect } from "react";

interface UseDocumentHotkeysOptions {
  onOpenPalette: () => void;
  onToggleSidebar: () => void;
}

export function useDocumentHotkeys({
  onOpenPalette,
  onToggleSidebar,
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
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onOpenPalette, onToggleSidebar]);
}
