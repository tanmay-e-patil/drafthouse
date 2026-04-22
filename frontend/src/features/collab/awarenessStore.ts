import { create } from "zustand";

const IDLE_MS = 30_000;
const EXPIRED_MS = 5 * 60_000;

export interface AwarenessPeer {
  clientId: number;
  name: string;
  color: string;
  lastActive: number;
}

interface AwarenessState {
  peers: AwarenessPeer[];
  localClientId: number | null;
  setPeers: (peers: AwarenessPeer[]) => void;
  setLocalClientId: (id: number) => void;
  isIdle: (peer: Pick<AwarenessPeer, "lastActive">) => boolean;
  isExpired: (peer: Pick<AwarenessPeer, "lastActive">) => boolean;
}

export const useAwarenessStore = create<AwarenessState>((set) => ({
  peers: [],
  localClientId: null,
  setPeers: (peers) => set({ peers }),
  setLocalClientId: (localClientId) => set({ localClientId }),
  isIdle: ({ lastActive }) => Date.now() - lastActive >= IDLE_MS,
  isExpired: ({ lastActive }) => Date.now() - lastActive >= EXPIRED_MS,
}));
