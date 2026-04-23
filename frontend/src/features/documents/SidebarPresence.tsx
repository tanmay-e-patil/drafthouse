import { Avatar, AvatarFallback } from "#/components/ui/avatar";
import type { DocumentPresencePeer } from "./api";

const MAX_VISIBLE = 3;
const IDLE_MS = 30_000;
const EXPIRED_MS = 5 * 60_000;

function initials(name: string): string {
  return name
    .split(/\s+/)
    .map((part) => part[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
}

function emailToName(email: string | null): string | null {
  return email?.split("@")[0] ?? null;
}

export default function SidebarPresence({
  peers,
  currentUserEmail,
  maxVisible = MAX_VISIBLE,
}: {
  peers: DocumentPresencePeer[];
  currentUserEmail: string | null;
  maxVisible?: number;
}) {
  const selfName = emailToName(currentUserEmail);
  const now = Date.now();
  const visible = peers.filter((peer) => {
    if (peer.name === selfName) return false;
    const lastActive = new Date(peer.last_active).getTime();
    return Number.isFinite(lastActive) && now - lastActive < EXPIRED_MS;
  });

  if (visible.length === 0) return null;

  const shown = visible.slice(0, maxVisible);
  const overflow = visible.length - shown.length;

  return (
    <div
      className="flex items-center -space-x-1.5"
      role="group"
      aria-label="Active collaborators"
      data-testid="sidebar-presence"
    >
      {shown.map((peer) => {
        const lastActive = new Date(peer.last_active).getTime();
        const idle = now - lastActive >= IDLE_MS;
        return (
          <Avatar
            key={`${peer.name}-${peer.last_active}`}
            className="size-5 border border-background text-[8px] font-semibold"
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
        <Avatar className="size-5 border border-background bg-muted text-[8px]">
          <AvatarFallback className="bg-muted text-muted-foreground">
            +{overflow}
          </AvatarFallback>
        </Avatar>
      )}
    </div>
  );
}
