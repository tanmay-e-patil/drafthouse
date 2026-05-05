import { useEffect, useRef } from "react";
import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";
import { yCollab } from "y-codemirror.next";
import { EditorView } from "@codemirror/view";
import { EditorState, type Extension } from "@codemirror/state";
import { issueWsTicket } from "./api";
import { useCollabStore } from "./store";
import { useAuthStore } from "#/features/auth/store";
import { decodeTitleUpdate } from "./titleUpdate";
import { assignColor } from "./awarenessColors";
import { useAwarenessStore, type AwarenessPeer } from "./awarenessStore";

const WS_BASE = import.meta.env.VITE_WS_URL ?? "ws://localhost:8080";

/** Maximum reconnection delay in ms (30 seconds). */
const MAX_RECONNECT_MS = 30_000;

function backoffDelay(attempt: number): number {
  const base = Math.min(MAX_RECONNECT_MS, 1000 * 2 ** attempt);
  return base * (0.75 + Math.random() * 0.5); // ±25% jitter
}

function emailToName(email: string): string {
  return email.split("@")[0] ?? email;
}

export interface UseCollabEditorOptions {
  docId: string;
  container: HTMLElement;
  extensions?: Extension[];
  initialContent?: string;
  readOnly?: boolean;
  /** Called when a remote title_update message (type 3) arrives. */
  onTitleUpdate?: (title: string) => void;
  onViewChange?: (view: EditorView | null) => void;
}

export interface CollabEditorHandle {
  destroy: () => void;
}

export function useCollabEditor(
  options: UseCollabEditorOptions | null
): React.MutableRefObject<CollabEditorHandle | null> {
  const handleRef = useRef<CollabEditorHandle | null>(null);
  const setStatus = useCollabStore((s) => s.setStatus);
  const accessToken = useAuthStore((s) => s.accessToken);
  const storedEmail = useAuthStore((s) => s.email);
  const setPeers = useAwarenessStore((s) => s.setPeers);
  const setLocalClientId = useAwarenessStore((s) => s.setLocalClientId);

  useEffect(() => {
    if (!options) return;
    const {
      docId,
      container,
      extensions = [],
      initialContent,
      readOnly = false,
      onTitleUpdate,
      onViewChange,
    } = options;

    let destroyed = false;
    let provider: WebsocketProvider | null = null;
    let view: EditorView | null = null;
    let reconnectAttempt = 0;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let userColor = "#3182CE";
    let userName = storedEmail ? emailToName(storedEmail) : "Anonymous";

    const ydoc = new Y.Doc();
    const ytext = ydoc.getText("content");

    function syncPeers(awareness: WebsocketProvider["awareness"]) {
      const states = awareness.getStates();
      // De-duplicate by name: same user refreshing gets a new clientID but same name.
      // Keep the entry with the highest lastActive (most recent session).
      const byName = new Map<string, AwarenessPeer>();
      states.forEach((state, clientId) => {
        const u = state["user"] as { name?: string; color?: string; lastActive?: number } | undefined;
        if (u?.name && u?.color) {
          const candidate: AwarenessPeer = {
            clientId,
            name: u.name,
            color: u.color,
            lastActive: u.lastActive ?? Date.now(),
          };
          const existing = byName.get(u.name);
          if (!existing || candidate.lastActive > existing.lastActive) {
            byName.set(u.name, candidate);
          }
        }
      });
      const peers = Array.from(byName.values());
      setPeers(peers);
    }

    async function connect() {
      if (destroyed) return;
      setStatus("connecting");

      let ticketParam: Record<string, string> = {};

      if (accessToken) {
        try {
          const { ticket } = await issueWsTicket(docId);
          ticketParam = { ticket };
        } catch {
          // Public viewers can connect without a ticket.
        }
      }

      provider = new WebsocketProvider(`${WS_BASE}/collab`, docId, ydoc, {
        connect: true,
        params: ticketParam,
        resyncInterval: -1,
      });

      const awareness = provider.awareness;

      // Assign color not already used by others in this room
      const usedColors = Array.from(awareness.getStates().values())
        .map((s) => {
          const u = s["user"] as { color?: string } | undefined;
          return u?.color ?? "";
        })
        .filter(Boolean);
      userColor = assignColor(usedColors);

      // Register local client ID so AvatarStrip can exclude self
      setLocalClientId(awareness.clientID);

      // Set local awareness state
      awareness.setLocalStateField("user", {
        name: userName,
        color: userColor,
        lastActive: Date.now(),
      });

      // Listen for awareness changes and sync to store
      const onAwarenessChange = () => syncPeers(awareness);
      awareness.on("change", onAwarenessChange);

      // Handle custom message type 3 (title_update) from server.
      if (onTitleUpdate) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (provider as any).messageHandlers[3] = (
          _encoder: unknown,
          decoder: { arr: Uint8Array; pos: number },
        ) => {
          const remaining = decoder.arr.subarray(decoder.pos);
          const full = new Uint8Array(1 + remaining.length);
          full[0] = 3;
          full.set(remaining, 1);
          const title = decodeTitleUpdate(full);
          if (title !== null) onTitleUpdate(title);
        };
      }

      provider.on("status", ({ status }: { status: string }) => {
        if (status === "connected") {
          reconnectAttempt = 0;
          setStatus("syncing");
        } else if (status === "disconnected") {
          setStatus("disconnected");
          scheduleReconnect();
        }
      });

      provider.on("sync", (synced: boolean) => {
        if (synced) {
          if (initialContent && ytext.toString() === "") {
            ydoc.transact(() => { ytext.insert(0, initialContent); });
          }
          setStatus("connected");
        }
      });

      // Build editor if not yet created
      if (!view) {
        // Update lastActive in awareness on any doc change (cursor/edit)
        const activityTracker = EditorView.updateListener.of((update) => {
          if (update.selectionSet || update.docChanged) {
            awareness.setLocalStateField("user", {
              name: userName,
              color: userColor,
              lastActive: Date.now(),
            });
          }
        });

        const state = EditorState.create({
          doc: ytext.toString(),
          extensions: [
            ...extensions,
            EditorView.editable.of(!readOnly),
            activityTracker,
            yCollab(ytext, awareness),
          ],
        });
        view = new EditorView({ state, parent: container });
        onViewChange?.(view);
      }
    }

    function scheduleReconnect() {
      if (destroyed) return;
      const delay = backoffDelay(reconnectAttempt++);
      reconnectTimer = setTimeout(() => {
        if (!destroyed) {
          provider?.destroy();
          provider = null;
          connect();
        }
      }, delay);
    }

    connect();

    handleRef.current = {
      destroy() {
        destroyed = true;
        if (reconnectTimer) clearTimeout(reconnectTimer);
        provider?.destroy();
        view?.destroy();
        onViewChange?.(null);
        setPeers([]);
      },
    };

    return () => {
      handleRef.current?.destroy();
      handleRef.current = null;
    };
  }, [options?.docId, options?.readOnly, accessToken, storedEmail]);

  return handleRef;
}
