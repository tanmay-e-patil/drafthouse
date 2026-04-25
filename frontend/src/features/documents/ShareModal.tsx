import { useState, useEffect } from "react";
import {
  type InviteLink,
  type DocumentMember,
  type MemberRole,
  createInviteLinkApi,
  listInviteLinksApi,
  revokeInviteLinkApi,
  listMembersApi,
  removeMemberApi,
  updateMemberRoleApi,
  updateDocumentApi,
} from "./api";
import { Button } from "#/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "#/components/ui/dialog";
import { Switch } from "#/components/ui/switch";
import { Separator } from "#/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { Copy, Check, Trash2, Link as LinkIcon, Globe } from "lucide-react";
import { toast } from "sonner";

interface ShareModalProps {
  docId: string;
  docTitle: string;
  isPublic: boolean;
  onClose: () => void;
  onPublicToggle: (isPublic: boolean) => void;
}

export function ShareModal({
  docId,
  docTitle,
  isPublic,
  onClose,
  onPublicToggle,
}: ShareModalProps) {
  const [members, setMembers] = useState<DocumentMember[]>([]);
  const [links, setLinks] = useState<InviteLink[]>([]);
  const [newRole, setNewRole] = useState<MemberRole>("editor");
  const [loading, setLoading] = useState(false);
  const [copiedToken, setCopiedToken] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, [docId]);

  async function loadData() {
    try {
      const [m, l] = await Promise.all([
        listMembersApi(docId),
        listInviteLinksApi(docId),
      ]);
      setMembers(m);
      setLinks(l);
    } catch {
      toast.error("Failed to load sharing data");
    }
  }

  async function handleGenerateLink() {
    setLoading(true);
    try {
      const link = await createInviteLinkApi(docId, { role: newRole });
      setLinks((prev) => [link, ...prev]);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to create link");
    } finally {
      setLoading(false);
    }
  }

  async function handleRevokeLink(token: string) {
    try {
      await revokeInviteLinkApi(docId, token);
      setLinks((prev) => prev.filter((l) => l.token !== token));
      toast.success("Invite link revoked");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to revoke link");
    }
  }

  async function handleRemoveMember(userId: string) {
    try {
      await removeMemberApi(docId, userId);
      setMembers((prev) => prev.filter((m) => m.user_id !== userId));
      toast.success("Member removed");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to remove member");
    }
  }

  async function handleRoleChange(userId: string, role: MemberRole) {
    try {
      const updated = await updateMemberRoleApi(docId, userId, role);
      setMembers((prev) =>
        prev.map((m) => (m.user_id === userId ? updated : m)),
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to update role");
    }
  }

  async function handlePublicToggle() {
    const next = !isPublic;
    try {
      await updateDocumentApi(docId, { is_public: next });
      onPublicToggle(next);
    } catch (e) {
      toast.error(
        e instanceof Error ? e.message : "Failed to update public access",
      );
    }
  }

  function copyToClipboard(token: string) {
    const url = `${window.location.origin}/invite/${token}`;
    navigator.clipboard.writeText(url);
    setCopiedToken(token);
    toast.success("Link copied");
    setTimeout(() => setCopiedToken(null), 2000);
  }

  const APP_ORIGIN =
    import.meta.env.VITE_APP_ORIGIN ?? window.location.origin;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="text-sm">Share &quot;{docTitle}&quot;</DialogTitle>
          <DialogDescription className="text-xs">
            Manage who has access to this document
          </DialogDescription>
        </DialogHeader>

        <div className="max-h-[60vh] space-y-4 overflow-y-auto">
          <section className="space-y-2">
            <h3 className="text-xs font-medium text-muted-foreground">
              People with access
            </h3>
            {members.length === 0 ? (
              <p className="text-xs text-muted-foreground">No members yet</p>
            ) : (
              <div className="space-y-1">
                {members.map((m) => (
                  <div
                    key={m.user_id}
                    className="flex items-center justify-between rounded-md px-2 py-1.5 transition-colors hover:bg-muted/60"
                  >
                    <span className="truncate text-xs text-muted-foreground">
                      {m.user_id}
                    </span>
                    <div className="flex items-center gap-1">
                      <Select
                        value={m.role}
                        onValueChange={(v) =>
                          handleRoleChange(m.user_id, v as MemberRole)
                        }
                      >
                        <SelectTrigger className="h-6 w-20 text-[11px]">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="editor">Editor</SelectItem>
                          <SelectItem value="viewer">Viewer</SelectItem>
                        </SelectContent>
                      </Select>
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        onClick={() => handleRemoveMember(m.user_id)}
                        className="text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 className="size-3" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>

          <Separator />

          <section className="space-y-2">
            <h3 className="text-xs font-medium text-muted-foreground">
              Invite link
            </h3>
            <div className="flex items-center gap-2">
              <Select
                value={newRole}
                onValueChange={(v) => setNewRole(v as MemberRole)}
              >
                <SelectTrigger className="h-7 w-24 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="editor">Editor</SelectItem>
                  <SelectItem value="viewer">Viewer</SelectItem>
                </SelectContent>
              </Select>
              <Button
                size="sm"
                className="text-xs"
                onClick={handleGenerateLink}
                disabled={loading}
              >
                <LinkIcon className="size-3" />
                Generate
              </Button>
            </div>
            {links.length > 0 && (
              <div className="space-y-1">
                {links.map((link) => {
                  const url = `${APP_ORIGIN}/invite/${link.token}`;
                  return (
                    <div
                      key={link.token}
                      className="flex items-center justify-between rounded-md border border-border/70 bg-muted/50 px-2 py-1.5"
                    >
                      <span className="truncate font-mono text-[11px] text-muted-foreground">
                        {url}
                      </span>
                      <div className="flex items-center gap-0.5 ml-2 shrink-0">
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          onClick={() => copyToClipboard(link.token)}
                        >
                          {copiedToken === link.token ? (
                            <Check className="size-3 text-accent-foreground" />
                          ) : (
                            <Copy className="size-3" />
                          )}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          onClick={() => handleRevokeLink(link.token)}
                          className="text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="size-3" />
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          <Separator />

          <section className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Globe className="size-3.5 text-muted-foreground" />
              <span className="text-xs">Public access</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-muted-foreground">
                {isPublic ? "Anyone with link" : "Off"}
              </span>
              <Switch checked={isPublic} onCheckedChange={handlePublicToggle} />
            </div>
          </section>
        </div>
      </DialogContent>
    </Dialog>
  );
}
