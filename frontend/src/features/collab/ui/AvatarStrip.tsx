import { useAwarenessStore } from "../awarenessStore";
import { Avatar, AvatarFallback } from "#/components/ui/avatar";

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
    (p) => p.clientId !== localClientId && now - p.lastActive < EXPIRED_MS,
  );

  if (visible.length === 0) return null;

  const shown = visible.slice(0, MAX_VISIBLE);
  const overflow = visible.length - shown.length;

  return (
    <div
      className="flex items-center -space-x-1.5"
      role="group"
      aria-label="Active collaborators"
    >
      {shown.map((peer) => {
        const idle = now - peer.lastActive >= IDLE_MS;
        return (
          <Avatar
            key={peer.clientId}
            className="size-6 border-2 border-background text-[9px] font-semibold"
            style={{
              backgroundColor: peer.color,
              opacity: idle ? 0.35 : 1,
            }}
            title={peer.name}
          >
            <AvatarFallback className="bg-transparent text-white">
              {initials(peer.name)}
            </AvatarFallback>
          </Avatar>
        );
      })}
      {overflow > 0 && (
        <Avatar className="size-6 border-2 border-background bg-muted text-[9px]">
          <AvatarFallback className="bg-muted text-muted-foreground">
            +{overflow}
          </AvatarFallback>
        </Avatar>
      )}
    </div>
  );
}
