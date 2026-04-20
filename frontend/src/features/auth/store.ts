import { create } from "zustand";
import { refreshApi } from "./api";

interface AuthState {
  accessToken: string | null;
  hydrated: boolean;
  setAccessToken: (token: string) => void;
  clearAuth: () => void;
  hydrate: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  accessToken: null,
  hydrated: false,
  setAccessToken: (token) => set({ accessToken: token }),
  clearAuth: () => set({ accessToken: null }),
  hydrate: async () => {
    try {
      const data = await refreshApi();
      set({ accessToken: data.access_token, hydrated: true });
    } catch {
      set({ accessToken: null, hydrated: true });
    }
  },
}));
