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
import { Input } from "#/components/ui/input";
import { Label } from "#/components/ui/label";

export function SettingsDeleteDialog({
  open,
  password,
  error,
  deleting,
  onOpenChange,
  onPasswordChange,
  onDeleteAccount,
}: {
  open: boolean;
  password: string;
  error: string | null;
  deleting: boolean;
  onOpenChange: (open: boolean) => void;
  onPasswordChange: (password: string) => void;
  onDeleteAccount: () => void;
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
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
            value={password}
            onChange={(event) => onPasswordChange(event.target.value)}
          />
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={deleting}>Cancel</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            onClick={onDeleteAccount}
            disabled={deleting}
          >
            {deleting ? "Deleting..." : "Delete account"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
