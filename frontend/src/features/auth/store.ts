import { create } from "zustand";
import { refreshApi } from "./api";

const EMAIL_KEY = "dh_user_email";

function readEmail(): string | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage.getItem(EMAIL_KEY) : null;
  } catch {
    return null;
  }
}

interface AuthState {
  accessToken: string | null;
  email: string | null;
  hydrated: boolean;
  setAccessToken: (token: string) => void;
  setEmail: (email: string) => void;
  clearAuth: () => void;
  hydrate: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  accessToken: null,
  email: readEmail(),
  hydrated: false,
  setAccessToken: (token) => set({ accessToken: token }),
  setEmail: (email) => {
    try {
      localStorage.setItem(EMAIL_KEY, email);
    } catch {}
    set({ email });
  },
  clearAuth: () => {
    try {
      localStorage.removeItem(EMAIL_KEY);
    } catch {}
    set({ accessToken: null, email: null });
  },
  hydrate: async () => {
    try {
      const data = await refreshApi();
      set({ accessToken: data.access_token, hydrated: true });
    } catch {
      set({ accessToken: null, hydrated: true });
    }
  },
}));
