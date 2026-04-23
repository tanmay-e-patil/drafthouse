import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";
import Sidebar from "#/components/Sidebar";
import { Button } from "#/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";
import { Input } from "#/components/ui/input";
import { Label } from "#/components/ui/label";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/components/ui/alert-dialog";
import {
  changePasswordApi,
  deleteAccountApi,
  exportAccountDataApi,
  getMeApi,
  type MeResponse,
} from "#/features/auth/api";
import { useAuthStore } from "#/features/auth/store";

const MIN_PASSWORD_LENGTH = 8;

export const Route = createFileRoute("/settings")({ component: SettingsPage });

export function SettingsPage() {
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const clearAuth = useAuthStore((s) => s.clearAuth);

  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [profile, setProfile] = useState<MeResponse | null>(null);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [loadingProfile, setLoadingProfile] = useState(true);

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [passwordError, setPasswordError] = useState<string | null>(null);
  const [savingPassword, setSavingPassword] = useState(false);

  const [exporting, setExporting] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletePassword, setDeletePassword] = useState("");
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  useEffect(() => {
    if (!hydrated) return;
    if (!accessToken) {
      navigate({
        to: "/login",
        search: { redirect: "/settings" } as never,
        replace: true,
      });
    }
  }, [hydrated, accessToken, navigate]);

  useEffect(() => {
    if (!hydrated || !accessToken) return;

    let cancelled = false;
    setLoadingProfile(true);
    setProfileError(null);

    getMeApi(accessToken)
      .then((data) => {
        if (cancelled) return;
        setProfile(data);
        useAuthStore.getState().setEmail(data.email);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        const message =
          error instanceof Error ? error.message : "Failed to fetch profile";
        setProfileError(message);
        if (message.toLowerCase().includes("failed")) {
          clearAuth();
          navigate({
            to: "/login",
            search: { redirect: "/settings" } as never,
            replace: true,
          });
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingProfile(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [hydrated, accessToken, navigate, clearAuth]);

  const toggleSidebar = useCallback(
    () => setSidebarCollapsed((value) => !value),
    []
  );

  async function handlePasswordSubmit(event: FormEvent) {
    event.preventDefault();
    if (!accessToken) return;

    if (!currentPassword || !newPassword || !confirmPassword) {
      setPasswordError("All password fields are required");
      return;
    }
    if (newPassword.length < MIN_PASSWORD_LENGTH) {
      setPasswordError("Password must be at least 8 characters");
      return;
    }
    if (newPassword !== confirmPassword) {
      setPasswordError("New passwords do not match");
      return;
    }

    setPasswordError(null);
    setSavingPassword(true);

    try {
      await changePasswordApi(accessToken, currentPassword, newPassword);
      clearAuth();
      toast.success("Password updated. Sign in again to continue.");
      navigate({ to: "/login", replace: true });
    } catch (error: unknown) {
      setPasswordError(
        error instanceof Error ? error.message : "Failed to update password"
      );
    } finally {
      setSavingPassword(false);
    }
  }

  async function handleExport() {
    if (!accessToken) return;
    setExporting(true);
    try {
      await exportAccountDataApi(accessToken);
      toast.success("Export started. Check your email.");
    } catch (error: unknown) {
      toast.error(
        error instanceof Error ? error.message : "Failed to export account data"
      );
    } finally {
      setExporting(false);
    }
  }

  async function handleDeleteAccount() {
    if (!accessToken) return;
    if (!deletePassword) {
      setDeleteError("Current password is required");
      return;
    }

    setDeleteError(null);
    setDeleting(true);

    try {
      await deleteAccountApi(accessToken, deletePassword);
      clearAuth();
      toast.success("Account deleted successfully.");
      navigate({ to: "/", replace: true });
    } catch (error: unknown) {
      setDeleteError(
        error instanceof Error ? error.message : "Failed to delete account"
      );
    } finally {
      setDeleting(false);
    }
  }

  if (!hydrated) return null;

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
      <main className="flex flex-1 flex-col overflow-hidden">
        <div className="flex h-12 items-center border-b border-border px-4">
          <h1 className="text-sm font-medium">Settings</h1>
        </div>
        <div className="flex-1 overflow-y-auto p-4 sm:p-6">
          <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
            <Card>
              <CardHeader>
                <CardTitle>Account</CardTitle>
                <CardDescription>Your verified profile details.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-2">
                {loadingProfile ? (
                  <p className="text-sm text-muted-foreground">Loading profile...</p>
                ) : profileError ? (
                  <p className="text-sm text-destructive">{profileError}</p>
                ) : (
                  <div className="space-y-1.5">
                    <Label htmlFor="settings-email">Email</Label>
                    <Input
                      id="settings-email"
                      value={profile?.email ?? ""}
                      readOnly
                      aria-readonly="true"
                    />
                  </div>
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Change password</CardTitle>
                <CardDescription>
                  Updating your password signs you out on all devices.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <form className="space-y-3" onSubmit={handlePasswordSubmit}>
                  {passwordError && (
                    <p className="text-sm text-destructive">{passwordError}</p>
                  )}
                  <div className="space-y-1.5">
                    <Label htmlFor="current-password">Current password</Label>
                    <Input
                      id="current-password"
                      type="password"
                      value={currentPassword}
                      onChange={(event) => setCurrentPassword(event.target.value)}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="new-password">New password</Label>
                    <Input
                      id="new-password"
                      type="password"
                      value={newPassword}
                      onChange={(event) => setNewPassword(event.target.value)}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="confirm-password">Confirm new password</Label>
                    <Input
                      id="confirm-password"
                      type="password"
                      value={confirmPassword}
                      onChange={(event) => setConfirmPassword(event.target.value)}
                    />
                  </div>
                  <Button type="submit" disabled={savingPassword}>
                    {savingPassword ? "Updating..." : "Update password"}
                  </Button>
                </form>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Export data</CardTitle>
                <CardDescription>
                  Email yourself a ZIP of your owned Markdown documents.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Button onClick={handleExport} disabled={exporting}>
                  {exporting ? "Starting export..." : "Export all documents"}
                </Button>
              </CardContent>
            </Card>

            <Card className="border-destructive/40">
              <CardHeader>
                <CardTitle>Delete account</CardTitle>
                <CardDescription>
                  This permanently removes your account and owned documents.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Button
                  variant="destructive"
                  onClick={() => {
                    setDeleteDialogOpen(true);
                    setDeleteError(null);
                    setDeletePassword("");
                  }}
                >
                  Delete account
                </Button>
              </CardContent>
            </Card>
          </div>
        </div>
      </main>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete account?</AlertDialogTitle>
            <AlertDialogDescription>
              This action is irreversible. Enter your current password to confirm
              permanent account deletion.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="space-y-1.5">
            <Label htmlFor="delete-password">Current password</Label>
            <Input
              id="delete-password"
              type="password"
              value={deletePassword}
              onChange={(event) => setDeletePassword(event.target.value)}
            />
            {deleteError && <p className="text-sm text-destructive">{deleteError}</p>}
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={handleDeleteAccount}
              disabled={deleting}
            >
              {deleting ? "Deleting..." : "Delete account"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
