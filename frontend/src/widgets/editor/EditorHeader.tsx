import { type ConnectionStatus } from "#/features/collab/store";
import AvatarStrip from "#/features/collab/ui/AvatarStrip";
import { Button } from "#/components/ui/button";
import { Toggle } from "#/components/ui/toggle";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "#/components/ui/tooltip";
import { Code2, Eye } from "lucide-react";
import { EDITOR_ACTIONS } from "./editorActions";
import type { FormattingActionId } from "./formatting";

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  connecting: "Connecting...",
  connected: "Synced",
  syncing: "Syncing...",
  disconnected: "Offline",
};

const STATUS_DOT: Record<ConnectionStatus, string> = {
  connecting: "bg-primary",
  connected: "bg-emerald-500 dark:bg-emerald-400",
  syncing: "bg-primary",
  disconnected: "bg-destructive",
};

export default function EditorHeader({
  mode,
  readOnly,
  collabStatus,
  onModeChange,
  onToolbarAction,
}: {
  mode: "edit" | "preview";
  readOnly: boolean;
  collabStatus: ConnectionStatus;
  onModeChange: (mode: "edit" | "preview") => void;
  onToolbarAction: (actionId: FormattingActionId) => void;
}) {
  return (
    <div className="flex min-h-12 items-center gap-2 border-b border-border/80 bg-card px-2 py-2 shadow-xs">
      <div className="flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger
            render={
              <Toggle
                pressed={mode === "edit"}
                onPressedChange={() => onModeChange("edit")}
                size="sm"
                className="gap-1.5 text-xs"
              />
            }
          >
            <Code2 className="size-3.5" />
            Edit
          </TooltipTrigger>
          <TooltipContent>Edit mode</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger
            render={
              <Toggle
                pressed={mode === "preview"}
                onPressedChange={() => onModeChange("preview")}
                size="sm"
                className="gap-1.5 text-xs"
              />
            }
          >
            <Eye className="size-3.5" />
            Preview
          </TooltipTrigger>
          <TooltipContent>Preview mode</TooltipContent>
        </Tooltip>
      </div>

      {mode === "edit" && !readOnly && (
        <div
          className="flex flex-wrap items-center gap-1 border-l border-border/80 pl-2"
          data-testid="editor-toolbar"
        >
          {EDITOR_ACTIONS.map((action) => (
            <Tooltip key={action.id}>
              <TooltipTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    onClick={() => onToolbarAction(action.id)}
                    aria-label={action.label}
                  />
                }
              >
                {action.shortLabel}
              </TooltipTrigger>
              <TooltipContent>{action.label} · {action.shortcut}</TooltipContent>
            </Tooltip>
          ))}
        </div>
      )}

      <div className="ml-auto flex items-center gap-2">
        <AvatarStrip />
        <Tooltip>
          <TooltipTrigger
            render={
              <div className="flex items-center gap-1.5 rounded-full border border-border/70 bg-card/70 px-2 py-1 text-[11px] text-muted-foreground shadow-xs">
                <span className={`inline-block size-1.5 rounded-full ${STATUS_DOT[collabStatus]}`} />
                {STATUS_LABEL[collabStatus]}
              </div>
            }
          />
          <TooltipContent>
            {collabStatus === "connected"
              ? "Connected to server"
              : collabStatus === "disconnected"
                ? "Changes saved locally — will sync when reconnected"
                : STATUS_LABEL[collabStatus]}
          </TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}
