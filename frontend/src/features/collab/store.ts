import { create } from "zustand";

export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "syncing";

interface CollabState {
  status: ConnectionStatus;
  setStatus: (status: ConnectionStatus) => void;
}

export const useCollabStore = create<CollabState>((set) => ({
  status: "disconnected",
  setStatus: (status) => set({ status }),
}));
