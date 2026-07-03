import { Avatar, AvatarFallback } from "#/components/ui/avatar";
import { Button } from "#/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import ThemeToggle from "#/components/ThemeToggle";
import { LogOut, Settings } from "lucide-react";

export default function SidebarFooter({
  email,
  onOpenSettings,
  onLogout,
}: {
  email: string | null;
  onOpenSettings: () => void;
  onLogout: () => void;
}) {
  const initials = email
    ? email
        .split("@")[0]
        .slice(0, 2)
        .toUpperCase()
    : "??";

  return (
    <div className="flex items-center justify-between px-3 py-2">
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button variant="ghost" size="sm" className="gap-2 px-2">
              <Avatar className="size-6">
                <AvatarFallback className="text-[10px]">
                  {initials}
                </AvatarFallback>
              </Avatar>
              <span className="max-w-24 truncate text-xs">
                {email?.split("@")[0]}
              </span>
            </Button>
          }
        />
        <DropdownMenuContent align="start" className="w-48">
          <DropdownMenuItem onClick={onOpenSettings}>
            <Settings className="size-4" />
            Settings
          </DropdownMenuItem>
          <DropdownMenuItem onClick={onLogout}>
            <LogOut className="size-4" />
            Sign out
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ThemeToggle />
    </div>
  );
}
