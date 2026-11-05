import { FileText, Plus } from "lucide-react";
import { Button } from "#/components/ui/button";

export function DocumentListEmptyState({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center text-muted-foreground">
      <FileText className="mb-2 size-8 opacity-40" />
      <p className="text-xs">No documents yet</p>
      <Button variant="outline" size="sm" className="mt-3 gap-1.5" onClick={onCreate}>
        <Plus className="size-3.5" />
        Create document
      </Button>
    </div>
  );
}
