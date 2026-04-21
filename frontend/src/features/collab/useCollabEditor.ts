import { useEffect, useRef } from "react";
import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";
import { yCollab } from "y-codemirror.next";
import { EditorView } from "@codemirror/view";
import { EditorState, type Extension } from "@codemirror/state";
import { issueWsTicket } from "./api";
import { useCollabStore } from "./store";
import { useAuthStore } from "#/features/auth/store";

const WS_BASE = import.meta.env.VITE_WS_URL ?? "ws://localhost:8080";

/** Maximum reconnection delay in ms (30 seconds). */
const MAX_RECONNECT_MS = 30_000;

function backoffDelay(attempt: number): number {
  const base = Math.min(MAX_RECONNECT_MS, 1000 * 2 ** attempt);
  return base * (0.75 + Math.random() * 0.5); // ±25% jitter
}

export interface UseCollabEditorOptions {
  docId: string;
  container: HTMLElement;
  extensions?: Extension[];
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

  useEffect(() => {
    if (!options) return;
    const { docId, container, extensions = [] } = options;

    let destroyed = false;
    let provider: WebsocketProvider | null = null;
    let view: EditorView | null = null;
    let reconnectAttempt = 0;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

    const ydoc = new Y.Doc();
    const ytext = ydoc.getText("content");

    async function connect() {
      if (destroyed) return;
      setStatus("connecting");

      let ticketParam: Record<string, string> = {};

      if (accessToken) {
        try {
          const { ticket } = await issueWsTicket(docId);
          ticketParam = { ticket };
        } catch {
          // unauthenticated viewer — connect without ticket
        }
      }

      // serverUrl + '/' + roomname is how y-websocket builds the final URL
      provider = new WebsocketProvider(`${WS_BASE}/collab`, docId, ydoc, {
        connect: true,
        params: ticketParam,
        // Disable y-websocket's own reconnect; we handle it manually
        resyncInterval: -1,
      });

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
        if (synced) setStatus("connected");
      });

      // Build editor if not yet created
      if (!view) {
        const state = EditorState.create({
          doc: ytext.toString(),
          extensions: [
            ...extensions,
            yCollab(ytext, provider.awareness),
          ],
        });
        view = new EditorView({ state, parent: container });
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
      },
    };

    return () => {
      handleRef.current?.destroy();
      handleRef.current = null;
    };
  }, [options?.docId, accessToken]);

  return handleRef;
}
