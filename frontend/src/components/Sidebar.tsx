import { useCallback, useEffect, useRef } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import {
  listDocumentsApi,
  deleteDocumentApi,
  createDocumentApi,
  getDocumentPresenceApi,
} from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import { logoutApi } from "#/features/auth/api";
import type { Document } from "#/features/documents/api";
import SidebarPresence from "#/features/documents/SidebarPresence";
import { Button } from "#/components/ui/button";
import { ScrollArea } from "#/components/ui/scroll-area";
import { Separator } from "#/components/ui/separator";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import { Avatar, AvatarFallback } from "#/components/ui/avatar";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "#/components/ui/tooltip";
import ThemeToggle from "#/components/ThemeToggle";
import { notifyTransientError } from "#/shared/errors";
import {
  FileText,
  Plus,
  Trash2,
  MoreHorizontal,
  LogOut,
  Settings,
  PanelLeftClose,
  PanelLeft,
} from "lucide-react";

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

interface SidebarProps {
  collapsed: boolean;
  onToggleCollapse: () => void;
}

export default function Sidebar({ collapsed, onToggleCollapse }: SidebarProps) {
  const navigate = useNavigate();
  const params = useParams({ strict: false }) as { documentId?: string };
  const {
    documents,
    hasMore,
    nextCursor,
    isLoading,
    presenceByDocumentId,
    setDocuments,
    replacePresence,
    prependDocument,
    removeDocumentFromList,
    setLoading,
  } = useDocumentStore();

  const hydrated = useAuthStore((s) => s.hydrated);
  const accessToken = useAuthStore((s) => s.accessToken);
  const email = useAuthStore((s) => s.email);
  const clearAuth = useAuthStore((s) => s.clearAuth);
  const hasFetched = useRef(false);

  const fetchDocuments = useCallback(
    async (cursor?: string | null) => {
      setLoading(true);
      try {
        const resp = await listDocumentsApi(cursor);
        if (!cursor) {
          setDocuments(resp.data, resp.next_cursor, resp.has_more);
        } else {
          setDocuments(
            [...documents, ...resp.data],
            resp.next_cursor,
            resp.has_more,
          );
        }
      } catch (error) {
        notifyTransientError(error);
      } finally {
        setLoading(false);
      }
    },
    [setDocuments, setLoading, documents],
  );

  useEffect(() => {
    if (hydrated && accessToken && !hasFetched.current) {
      hasFetched.current = true;
      fetchDocuments();
    }
  }, [hydrated, accessToken]);

  useEffect(() => {
    if (!hydrated || !accessToken || documents.length === 0) {
      replacePresence({});
      return;
    }

    let cancelled = false;

    async function pollPresence() {
      const results = await Promise.allSettled(
        documents.map(async (doc) => ({
          id: doc.id,
          peers: (await getDocumentPresenceApi(doc.id)).data,
        }))
      );

      if (cancelled) return;

      const nextPresence = results.reduce<Record<string, typeof presenceByDocumentId[string]>>(
        (acc, result) => {
          if (result.status === "fulfilled") {
            acc[result.value.id] = result.value.peers;
          }
          return acc;
        },
        {}
      );

      replacePresence(nextPresence);
    }

    void pollPresence();
    const intervalId = window.setInterval(() => {
      void pollPresence();
    }, 3000);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [hydrated, accessToken, documents, replacePresence]);

  async function handleCreate() {
    try {
      const doc = await createDocumentApi();
      prependDocument(doc);
      navigate({
        to: "/documents/$documentId",
        params: { documentId: doc.id },
      });
    } catch (error) {
      notifyTransientError(error);
    }
  }

  async function handleDelete(doc: Document) {
    try {
      await deleteDocumentApi(doc.id);
      removeDocumentFromList(doc.id);
      if (params.documentId === doc.id) {
        navigate({ to: "/" });
      }
    } catch (error) {
      notifyTransientError(error);
    }
  }

  function handleLoadMore() {
    if (nextCursor && hasMore && !isLoading) {
      fetchDocuments(nextCursor);
    }
  }

  async function handleLogout() {
    try {
      await logoutApi();
    } catch {
    }
    clearAuth();
    navigate({ to: "/" });
  }

  const initials = email
    ? email
        .split("@")[0]
        .slice(0, 2)
        .toUpperCase()
    : "??";

  if (collapsed) {
    return (
      <aside className="flex h-screen w-14 flex-col border-r border-border bg-sidebar">
        <div className="flex h-12 items-center justify-center">
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
        <ScrollArea className="flex-1 px-2 pt-1">
          <div className="flex flex-col items-center gap-1">
            {documents.slice(0, 10).map((doc) => (
              <Tooltip key={doc.id}>
                <TooltipTrigger
                  render={
                    <button
                      className={`rounded-md p-2 transition-colors ${
                        params.documentId === doc.id
                          ? "bg-accent text-accent-foreground"
                          : "text-sidebar-foreground hover:bg-accent/50"
                      }`}
                      onClick={() =>
                        navigate({
                          to: "/documents/$documentId",
                          params: { documentId: doc.id },
                        })
                      }
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

  return (
    <aside className="flex h-screen w-60 flex-col border-r border-border bg-sidebar">
      <div className="flex h-12 items-center justify-between px-3">
        <span className="text-sm font-semibold tracking-tight text-sidebar-foreground">
          Drafthouse
        </span>
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                variant="ghost"
                size="icon"
                onClick={onToggleCollapse}
                className="size-7"
              />
            }
          >
            <PanelLeftClose className="size-3.5" />
          </TooltipTrigger>
          <TooltipContent side="right">Collapse sidebar</TooltipContent>
        </Tooltip>
      </div>
      <Separator />
      <div className="px-3 pt-2 pb-1">
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 text-muted-foreground"
          onClick={handleCreate}
        >
          <Plus className="size-4" />
          New document
        </Button>
      </div>
      <ScrollArea className="flex-1 px-2">
        {documents.length === 0 && !isLoading && (
          <div className="flex flex-col items-center justify-center py-12 text-center text-muted-foreground">
            <FileText className="mb-2 size-8 opacity-40" />
            <p className="text-xs">No documents yet</p>
            <Button
              variant="outline"
              size="sm"
              className="mt-3 gap-1.5"
              onClick={handleCreate}
            >
              <Plus className="size-3.5" />
              Create document
            </Button>
          </div>
        )}
        {documents.map((doc) => (
          <div key={doc.id} className="group flex items-center gap-1">
            <button
              className={`min-w-0 flex-1 truncate rounded-md px-2 py-1.5 text-left text-sm transition-colors ${
                params.documentId === doc.id
                  ? "bg-accent text-accent-foreground font-medium"
                  : "text-sidebar-foreground hover:bg-accent/50"
              }`}
              onClick={() =>
                navigate({
                  to: "/documents/$documentId",
                  params: { documentId: doc.id },
                })
              }
            >
              <span className="block truncate">{doc.title}</span>
              <span className="block text-[11px] text-muted-foreground">
                {formatRelativeTime(doc.updated_at)}
              </span>
            </button>
            <SidebarPresence
              peers={presenceByDocumentId[doc.id] ?? []}
              currentUserEmail={email}
            />
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    variant="ghost"
                    size="icon"
                    className="mr-1 size-7 opacity-0 group-hover:opacity-100"
                  />
                }
              >
                <MoreHorizontal className="size-3.5" />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-36">
                <DropdownMenuItem
                  className="text-destructive focus:text-destructive"
                  onClick={() => handleDelete(doc)}
                >
                  <Trash2 className="size-3.5" />
                  Delete
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        ))}
        {hasMore && (
          <Button
            variant="ghost"
            size="sm"
            className="mt-1 w-full text-xs text-muted-foreground"
            onClick={handleLoadMore}
            disabled={isLoading}
          >
            {isLoading ? "Loading..." : "Load more"}
          </Button>
        )}
      </ScrollArea>
      <Separator />
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
            <DropdownMenuItem onClick={() => navigate({ to: "/settings" })}>
              <Settings className="size-4" />
              Settings
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleLogout}>
              <LogOut className="size-4" />
              Sign out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <ThemeToggle />
      </div>
    </aside>
  );
}
