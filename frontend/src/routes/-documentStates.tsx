import Sidebar from "#/components/Sidebar";
import { CommandPalette } from "#/features/documents/CommandPalette";

export function DocumentLoadingState({
  documentId,
  paletteOpen,
  sidebarCollapsed,
  focusMode,
  onPaletteOpenChange,
  onToggleSidebar,
}: {
  documentId: string;
  paletteOpen: boolean;
  sidebarCollapsed: boolean;
  focusMode: boolean;
  onPaletteOpenChange: (open: boolean) => void;
  onToggleSidebar: () => void;
}) {
  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <CommandPalette
        currentDocumentId={documentId}
        open={paletteOpen}
        onOpenChange={onPaletteOpenChange}
      />
      {!focusMode && (
        <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={onToggleSidebar} />
      )}
      <main className="flex flex-1 items-center justify-center text-muted-foreground">
        <p className="text-sm">Loading...</p>
      </main>
    </div>
  );
}

export function InaccessibleDocumentState({
  authRequired,
  sidebarCollapsed,
  onToggleSidebar,
}: {
  authRequired: boolean;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}) {
  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={onToggleSidebar} />
      <main className="flex flex-1 items-center justify-center p-8">
        <div className="ambient-panel max-w-sm rounded-3xl border border-border/80 p-8 text-center shadow-lg">
          <h1 className="font-heading text-xl font-semibold tracking-tight">Document unavailable</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            {authRequired
              ? "This document is private. You need an invite link to access it."
              : "This document was deleted, or you do not have access to it."}
          </p>
        </div>
      </main>
    </div>
  );
}
