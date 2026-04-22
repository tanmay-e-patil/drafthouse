import { useAwarenessStore } from "../awarenessStore";

const MAX_VISIBLE = 5;
const IDLE_MS = 30_000;
const EXPIRED_MS = 5 * 60_000;

function initials(name: string): string {
  return name
    .split(/\s+/)
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
}

export default function AvatarStrip() {
  const peers = useAwarenessStore((s) => s.peers);
  const localClientId = useAwarenessStore((s) => s.localClientId);
  const now = Date.now();

  const visible = peers.filter(
    (p) =>
      p.clientId !== localClientId &&
      now - p.lastActive < EXPIRED_MS
  );

  if (visible.length === 0) return null;

  const shown = visible.slice(0, MAX_VISIBLE);
  const overflow = visible.length - shown.length;

  return (
    <div className="avatar-strip" role="group" aria-label="Active collaborators">
      {shown.map((peer) => {
        const idle = now - peer.lastActive >= IDLE_MS;
        return (
          <span
            key={peer.clientId}
            className={`avatar${idle ? " idle" : ""}`}
            title={peer.name}
            style={{ backgroundColor: peer.color }}
            aria-label={peer.name}
          >
            {initials(peer.name)}
          </span>
        );
      })}
      {overflow > 0 && (
        <span className="avatar avatar-overflow" aria-label={`${overflow} more`}>
          +{overflow}
        </span>
      )}
    </div>
  );
}
