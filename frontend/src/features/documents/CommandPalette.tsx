import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Dialog, DialogContent } from "#/components/ui/dialog";
import { Input } from "#/components/ui/input";
import { listDocumentsApi, type Document } from "./api";
import { useDocumentStore } from "./store";
import { cn } from "#/lib/utils";

interface CommandPaletteProps {
  currentDocumentId?: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function mergeDocuments(existing: Document[], incoming: Document[]) {
  const map = new Map(existing.map((doc) => [doc.id, doc]));
  for (const doc of incoming) {
    map.set(doc.id, doc);
  }
  return Array.from(map.values()).sort(
    (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
  );
}

export function CommandPalette({
  currentDocumentId,
  open,
  onOpenChange,
}: CommandPaletteProps) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const documents = useDocumentStore((s) => s.documents);
  const hasMore = useDocumentStore((s) => s.hasMore);
  const nextCursor = useDocumentStore((s) => s.nextCursor);
  const setDocuments = useDocumentStore((s) => s.setDocuments);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActiveIndex(0);
      queueMicrotask(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    if (documents.length > 0 && !hasMore) return;

    let cancelled = false;

    async function loadAllDocuments() {
      setLoading(true);
      try {
        let merged = [...useDocumentStore.getState().documents];
        let cursor = merged.length === 0 ? null : nextCursor;
        let shouldLoadFirstPage = merged.length === 0;
        let hasMorePages = shouldLoadFirstPage || useDocumentStore.getState().hasMore;

        while (hasMorePages && !cancelled) {
          const response = await listDocumentsApi(shouldLoadFirstPage ? undefined : cursor, 100);
          merged = mergeDocuments(merged, response.data);
          cursor = response.next_cursor;
          hasMorePages = response.has_more;
          shouldLoadFirstPage = false;
        }

        if (!cancelled) {
          setDocuments(merged, cursor, hasMorePages);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadAllDocuments().catch(() => {
      if (!cancelled) setLoading(false);
    });

    return () => {
      cancelled = true;
    };
  }, [documents.length, hasMore, nextCursor, open, setDocuments]);

  const filteredDocuments = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    const matching = normalizedQuery
      ? documents.filter((doc) => doc.title.toLowerCase().includes(normalizedQuery))
      : documents;

    return [...matching].sort((a, b) => {
      if (a.id === currentDocumentId && b.id !== currentDocumentId) return -1;
      if (b.id === currentDocumentId && a.id !== currentDocumentId) return 1;
      return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
    });
  }, [currentDocumentId, documents, query]);

  useEffect(() => {
    setActiveIndex((current) =>
      filteredDocuments.length === 0 ? 0 : Math.min(current, filteredDocuments.length - 1),
    );
  }, [filteredDocuments.length]);

  function handleSelect(document: Document) {
    onOpenChange(false);
    navigate({
      to: "/documents/$documentId",
      params: { documentId: document.id },
    });
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) =>
        filteredDocuments.length === 0 ? 0 : Math.min(current + 1, filteredDocuments.length - 1),
      );
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => Math.max(current - 1, 0));
    } else if (event.key === "Enter") {
      const selected = filteredDocuments[activeIndex];
      if (selected) {
        event.preventDefault();
        handleSelect(selected);
      }
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl p-0" showCloseButton={false}>
        <div className="border-b border-border/80 bg-muted/35 p-3">
          <Input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search documents by title"
            aria-label="Search documents"
          />
        </div>

        <div className="max-h-80 overflow-y-auto p-2" data-testid="command-palette-results">
          {loading && (
            <p className="px-2 py-6 text-sm text-muted-foreground">Loading documents...</p>
          )}

          {!loading && filteredDocuments.length === 0 && (
            <p className="px-2 py-6 text-sm text-muted-foreground">No matching documents</p>
          )}

          {!loading && filteredDocuments.map((doc, index) => (
            <button
              key={doc.id}
              type="button"
              onClick={() => handleSelect(doc)}
              className={cn(
                "flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-sm transition-all duration-200",
                index === activeIndex ? "bg-accent text-accent-foreground shadow-sm" : "hover:-translate-y-px hover:bg-muted",
              )}
            >
              <span className="truncate">{doc.title}</span>
              {doc.id === currentDocumentId && (
                <span className="ml-3 shrink-0 text-[11px] text-muted-foreground">Current</span>
              )}
            </button>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
