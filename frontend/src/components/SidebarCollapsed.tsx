import type { Document } from "#/features/documents/api";
import { Button } from "#/components/ui/button";
import { ScrollArea } from "#/components/ui/scroll-area";
import { Separator } from "#/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "#/components/ui/tooltip";
import { FileText, PanelLeft } from "lucide-react";

export default function SidebarCollapsed({
  documents,
  selectedDocumentId,
  onSelectDocument,
  onToggleCollapse,
}: {
  documents: Document[];
  selectedDocumentId?: string;
  onSelectDocument: (documentId: string) => void;
  onToggleCollapse: () => void;
}) {
  return (
    <aside className="flex h-screen w-14 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar/95 shadow-sm shadow-foreground/5 backdrop-blur">
      <div className="flex h-12 shrink-0 items-center justify-center">
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                variant="ghost"
                size="icon"
                onClick={onToggleCollapse}
                className="size-8"
              />
            }
          >
            <PanelLeft className="size-4" />
          </TooltipTrigger>
          <TooltipContent side="right">Expand sidebar</TooltipContent>
        </Tooltip>
      </div>
      <Separator />
      <ScrollArea className="min-h-0 flex-1 px-2 pt-1">
        <div className="flex flex-col items-center gap-1">
          {documents.slice(0, 10).map((doc) => (
            <Tooltip key={doc.id}>
              <TooltipTrigger
                render={
                  <button
                    type="button"
                    aria-label={doc.title}
                    className={`rounded-md p-2 transition-colors ${
                      selectedDocumentId === doc.id
                        ? "bg-sidebar-accent text-sidebar-accent-foreground shadow-sm"
                        : "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
                    }`}
                    onClick={() => onSelectDocument(doc.id)}
                  />
                }
              >
                <FileText className="size-4" />
              </TooltipTrigger>
              <TooltipContent side="right">{doc.title}</TooltipContent>
            </Tooltip>
          ))}
        </div>
      </ScrollArea>
    </aside>
  );
}
