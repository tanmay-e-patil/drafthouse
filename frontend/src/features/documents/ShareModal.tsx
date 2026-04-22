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
  const [error, setError] = useState<string | null>(null);

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
      setError("Failed to load sharing data");
    }
  }

  async function handleGenerateLink() {
    setLoading(true);
    setError(null);
    try {
      const link = await createInviteLinkApi(docId, { role: newRole });
      setLinks((prev) => [link, ...prev]);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create link");
    } finally {
      setLoading(false);
    }
  }

  async function handleRevokeLink(token: string) {
    try {
      await revokeInviteLinkApi(docId, token);
      setLinks((prev) => prev.filter((l) => l.token !== token));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to revoke link");
    }
  }

  async function handleRemoveMember(userId: string) {
    try {
      await removeMemberApi(docId, userId);
      setMembers((prev) => prev.filter((m) => m.user_id !== userId));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to remove member");
    }
  }

  async function handleRoleChange(userId: string, role: MemberRole) {
    try {
      const updated = await updateMemberRoleApi(docId, userId, role);
      setMembers((prev) =>
        prev.map((m) => (m.user_id === userId ? updated : m))
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to update role");
    }
  }

  async function handlePublicToggle() {
    const next = !isPublic;
    try {
      await updateDocumentApi(docId, { is_public: next });
      onPublicToggle(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to update public access");
    }
  }

  function copyToClipboard(token: string) {
    const url = `${window.location.origin}/invite/${token}`;
    navigator.clipboard.writeText(url);
    setCopiedToken(token);
    setTimeout(() => setCopiedToken(null), 2000);
  }

  const APP_ORIGIN = import.meta.env.VITE_APP_ORIGIN ?? window.location.origin;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        style={{
          background: "var(--bg, #fff)",
          borderRadius: 8,
          padding: 24,
          width: 420,
          maxWidth: "90vw",
          maxHeight: "80vh",
          overflowY: "auto",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: 16,
          }}
        >
          <h2 style={{ margin: 0, fontSize: 18 }}>Share "{docTitle}"</h2>
          <button onClick={onClose} aria-label="Close">✕</button>
        </div>

        {error && (
          <p style={{ color: "red", marginBottom: 12, fontSize: 13 }}>{error}</p>
        )}

        <section style={{ marginBottom: 20 }}>
          <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>
            People with access
          </h3>
          {members.length === 0 ? (
            <p style={{ color: "#888", fontSize: 13 }}>No members yet</p>
          ) : (
            members.map((m) => (
              <div
                key={m.user_id}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: 6,
                }}
              >
                <span style={{ fontSize: 13, fontFamily: "monospace" }}>
                  {m.user_id.slice(0, 8)}…
                </span>
                <div style={{ display: "flex", gap: 6 }}>
                  <select
                    value={m.role}
                    onChange={(e) =>
                      handleRoleChange(m.user_id, e.target.value as MemberRole)
                    }
                    style={{ fontSize: 12 }}
                  >
                    <option value="editor">Editor</option>
                    <option value="viewer">Viewer</option>
                  </select>
                  <button
                    onClick={() => handleRemoveMember(m.user_id)}
                    aria-label="Remove member"
                    style={{ fontSize: 12 }}
                  >
                    ✕
                  </button>
                </div>
              </div>
            ))
          )}
        </section>

        <section style={{ marginBottom: 20 }}>
          <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>
            Invite link
          </h3>
          <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
            <select
              value={newRole}
              onChange={(e) => setNewRole(e.target.value as MemberRole)}
              style={{ fontSize: 13 }}
            >
              <option value="editor">Editor</option>
              <option value="viewer">Viewer</option>
            </select>
            <button
              onClick={handleGenerateLink}
              disabled={loading}
              style={{ fontSize: 13 }}
            >
              {loading ? "Generating…" : "Generate Link"}
            </button>
          </div>
          {links.map((link) => {
            const url = `${APP_ORIGIN}/invite/${link.token}`;
            return (
              <div
                key={link.token}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: 6,
                  fontSize: 12,
                }}
              >
                <span style={{ fontFamily: "monospace", flex: 1, overflow: "hidden", textOverflow: "ellipsis" }}>
                  {url}
                </span>
                <div style={{ display: "flex", gap: 4, marginLeft: 8 }}>
                  <button
                    onClick={() => copyToClipboard(link.token)}
                    aria-label="Copy link"
                    style={{ fontSize: 11 }}
                  >
                    {copiedToken === link.token ? "✓" : "📋"}
                  </button>
                  <button
                    onClick={() => handleRevokeLink(link.token)}
                    aria-label="Revoke link"
                    style={{ fontSize: 11 }}
                  >
                    ✕
                  </button>
                </div>
              </div>
            );
          })}
        </section>

        <section>
          <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>
            Public access
          </h3>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, cursor: "pointer" }}>
            <input
              type="checkbox"
              checked={isPublic}
              onChange={handlePublicToggle}
            />
            Anyone with link can view
          </label>
        </section>
      </div>
    </div>
  );
}
