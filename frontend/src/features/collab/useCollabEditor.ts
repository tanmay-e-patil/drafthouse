import { useEffect, useRef } from "react";
import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";
import { yCollab } from "y-codemirror.next";
import type { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
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

/**
 * One collaboration session per document (#11–#14, REG-SELF).
 *
 * - The session survives ordinary parent re-renders: callbacks are read
 *   through the session's latest options instead of re-creating providers
 *   (#12). Teardown on cleanup is deferred by a tick so a following effect
 *   (the re-render handoff) cancels it, while a real unmount lets it fire.
 * - Every in-flight connect captures an attempt number; anything superseded
 *   (re-render, reconnect, unmount) bails before creating resources, so no
 *   orphan sockets/editors remain (#11, REG-SELF).
 * - Reconnection has one owner: a pending timer is cleared when the provider
 *   reports connected again and never stacks (#14).
 * - Replacing a provider rebuilds the editor binding so yCollab always
 *   tracks the current awareness (#13).
 */
interface Session {
  docId: string;
  ydoc: Y.Doc;
  ytext: Y.Text;
  userName: string;
  latest: UseCollabEditorOptions;
  provider: WebsocketProvider | null;
  view: EditorView | null;
  /** Supersede counter: in-flight connects bail once it moves past them. */
  attempt: number;
  destroyed: boolean;
  reconnectAttempt: number;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
}

export function useCollabEditor(
  options: UseCollabEditorOptions | null
): React.MutableRefObject<CollabEditorHandle | null> {
  const handleRef = useRef<CollabEditorHandle | null>(null);
  const sessionRef = useRef<Session | null>(null);
  const teardownTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const setStatus = useCollabStore((s) => s.setStatus);
  const accessToken = useAuthStore((s) => s.accessToken);
  const storedEmail = useAuthStore((s) => s.email);
  const setPeers = useAwarenessStore((s) => s.setPeers);
  const setLocalClientId = useAwarenessStore((s) => s.setLocalClientId);

  function cancelTeardown() {
    if (teardownTimerRef.current) {
      clearTimeout(teardownTimerRef.current);
      teardownTimerRef.current = null;
    }
  }

  function teardownSession() {
    cancelTeardown();
    const session = sessionRef.current;
    if (!session) return;
    session.destroyed = true;
    session.attempt++;
    if (session.reconnectTimer) {
      clearTimeout(session.reconnectTimer);
      session.reconnectTimer = null;
    }
    session.provider?.destroy();
    session.provider = null;
    session.view?.destroy();
    session.view = null;
    session.latest.onViewChange?.(null);
    setPeers([]);
  }

  function syncPeers(session: Session, awareness: WebsocketProvider["awareness"]) {
    const states = awareness.getStates();
    const peers: AwarenessPeer[] = [];
    states.forEach((state, clientId) => {
      // Never show the local session as a collaborator avatar.
      if (clientId === awareness.clientID) return;
      const u = state["user"] as
        | { name?: string; color?: string; lastActive?: number }
        | undefined;
      if (!u?.name || !u?.color) return;
      // Another session of the local user is still "self" (REG-SELF).
      if (session.userName !== "Anonymous" && u.name === session.userName) return;
      peers.push({
        clientId,
        name: u.name,
        color: u.color,
        lastActive: u.lastActive ?? Date.now(),
      });
    });
    setPeers(peers);
  }

  function scheduleReconnect(session: Session) {
    if (session.destroyed) return;
    if (session.reconnectTimer) return; // one owner, never stacked (#14)
    const delay = backoffDelay(session.reconnectAttempt++);
    session.reconnectTimer = setTimeout(() => {
      session.reconnectTimer = null;
      if (session.destroyed) return;
      session.provider?.destroy();
      session.provider = null;
      void connect(session);
    }, delay);
  }

  async function connect(session: Session) {
    const attempt = ++session.attempt;
    const stale = () => session.destroyed || attempt !== session.attempt;
    const { docId } = session;
    setStatus("connecting");

    let params: Record<string, string> = {};
    const accessToken = useAuthStore.getState().accessToken;
    if (accessToken) {
      try {
        const { ticket } = await issueWsTicket(docId);
        if (stale()) return;
        params = { ticket };
      } catch {
        // An authenticated session must never silently fall back to an
        // anonymous socket (#17): a private document would surface as
        // read-only/public instead of reporting the auth failure.
        if (!stale()) {
          setStatus("disconnected");
          scheduleReconnect(session);
        }
        return;
      }
    }

    const [{ EditorView }, { EditorState, StateEffect }] = await Promise.all([
      import("@codemirror/view"),
      import("@codemirror/state"),
    ]);
    if (stale()) return;

    const provider = new WebsocketProvider(`${WS_BASE}/collab`, docId, session.ydoc, {
      connect: true,
      params,
      resyncInterval: -1,
    });
    if (stale()) {
      provider.destroy();
      return;
    }
    session.provider = provider;

    const awareness = provider.awareness;
    const usedColors = Array.from(awareness.getStates().values()).flatMap((s) => {
      const u = s["user"] as { color?: string } | undefined;
      return u?.color ? [u.color] : [];
    });
    const userColor = assignColor(usedColors);

    // Register local client ID so AvatarStrip can exclude self
    setLocalClientId(awareness.clientID);

    awareness.setLocalStateField("user", {
      name: session.userName,
      color: userColor,
      lastActive: Date.now(),
    });

    const onAwarenessChange = () => syncPeers(session, awareness);
    awareness.on("change", onAwarenessChange);

    // Handle custom message type 3 (title_update) from server.
    provider.messageHandlers[3] = (
      _encoder: unknown,
      decoder: { arr: Uint8Array; pos: number },
    ) => {
      const remaining = decoder.arr.subarray(decoder.pos);
      const full = new Uint8Array(1 + remaining.length);
      full[0] = 3;
      full.set(remaining, 1);
      const title = decodeTitleUpdate(full);
      if (title !== null) session.latest.onTitleUpdate?.(title);
    };

    provider.on("status", ({ status }: { status: string }) => {
      if (status === "connected") {
        session.reconnectAttempt = 0;
        if (session.reconnectTimer) {
          clearTimeout(session.reconnectTimer);
          session.reconnectTimer = null;
        }
        setStatus("syncing");
      } else if (status === "disconnected") {
        setStatus("disconnected");
        scheduleReconnect(session);
      }
    });

    provider.on("sync", (synced: boolean) => {
      if (synced) setStatus("connected");
    });

    // A replaced provider has a new awareness. Reconfigure the existing view
    // in place so the yCollab binding always tracks the current awareness
    // (#13) without discarding the editor, its document, or selection.
    const { extensions = [], readOnly = false } = session.latest;
    const activityTracker = EditorView.updateListener.of((update) => {
      if (update.selectionSet || update.docChanged) {
        awareness.setLocalStateField("user", {
          name: session.userName,
          color: userColor,
          lastActive: Date.now(),
        });
      }
    });
    const collabExtensions: Extension[] = [
      ...extensions,
      EditorView.editable.of(!readOnly),
      activityTracker,
      yCollab(session.ytext, awareness),
    ];
    if (session.view) {
      session.view.dispatch({
        effects: StateEffect.reconfigure.of(collabExtensions),
      });
    } else {
      const state = EditorState.create({
        doc: session.ytext.toString(),
        extensions: collabExtensions,
      });
      session.view = new EditorView({
        state,
        parent: session.latest.container,
      });
      session.latest.onViewChange?.(session.view);
    }
  }

  useEffect(() => {
    cancelTeardown();

    if (!options) {
      teardownSession();
      return;
    }

    let session = sessionRef.current;
    if (!session || session.docId !== options.docId || session.destroyed) {
      teardownSession();
      session = {
        docId: options.docId,
        ydoc: new Y.Doc(),
        ytext: null!,
        userName: storedEmail ? emailToName(storedEmail) : "Anonymous",
        latest: options,
        provider: null,
        view: null,
        attempt: 0,
        destroyed: false,
        reconnectAttempt: 0,
        reconnectTimer: null,
      };
      session.ytext = session.ydoc.getText("content");
      sessionRef.current = session;
    }

    session.latest = options;
    // Supersede any in-flight connect from a previous effect run (#11).
    session.attempt++;
    if (!session.provider) {
      void connect(session);
    }

    return () => {
      // Deferred teardown: the next effect (re-render handoff) cancels it
      // and keeps the session alive (#12); a real unmount lets it fire.
      session!.attempt++;
      if (!teardownTimerRef.current) {
        teardownTimerRef.current = setTimeout(() => {
          teardownTimerRef.current = null;
          teardownSession();
        }, 0);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accessToken, options, setStatus, setLocalClientId, setPeers, storedEmail]);

  handleRef.current = {
    destroy: () => teardownSession(),
  };

  return handleRef;
}
