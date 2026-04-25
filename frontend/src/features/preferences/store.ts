import { create } from "zustand";

export type EditorFont = "inter" | "lora" | "jetbrains-mono";

const FONT_KEY = "dh_editor_font";

export const EDITOR_FONT_OPTIONS: Array<{
  value: EditorFont;
  label: string;
  className: string;
}> = [
  { value: "inter", label: "Inter", className: "font-sans" },
  { value: "lora", label: "Lora", className: "font-serif" },
  { value: "jetbrains-mono", label: "JetBrains Mono", className: "font-mono" },
];

function isEditorFont(value: string | null): value is EditorFont {
  return value === "inter" || value === "lora" || value === "jetbrains-mono";
}

function readStoredFont(): EditorFont {
  try {
    const stored = typeof localStorage !== "undefined" ? localStorage.getItem(FONT_KEY) : null;
    return isEditorFont(stored) ? stored : "inter";
  } catch {
    return "inter";
  }
}

interface PreferencesState {
  focusMode: boolean;
  editorFont: EditorFont;
  setFocusMode: (focusMode: boolean) => void;
  toggleFocusMode: () => void;
  setEditorFont: (font: EditorFont) => void;
}

export const usePreferencesStore = create<PreferencesState>((set) => ({
  focusMode: false,
  editorFont: readStoredFont(),
  setFocusMode: (focusMode) => set({ focusMode }),
  toggleFocusMode: () => set((state) => ({ focusMode: !state.focusMode })),
  setEditorFont: (font) => {
    try {
      localStorage.setItem(FONT_KEY, font);
    } catch {}
    set({ editorFont: font });
  },
}));
