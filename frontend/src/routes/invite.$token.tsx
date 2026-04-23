import { createFileRoute, useNavigate, useParams } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useAuthStore } from "#/features/auth/store";
import { acceptInviteApi } from "#/features/documents/api";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";

export const Route = createFileRoute("/invite/$token")({ component: AcceptInvite });

function AcceptInvite() {
  const { token } = useParams({ strict: false }) as { token: string };
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  useEffect(() => {
    if (!hydrated) return;
    if (!accessToken) {
      navigate({ to: "/login", search: { redirect: `/invite/${token}` } });
      return;
    }

    acceptInviteApi(token)
      .then((member) => {
        navigate({
          to: "/documents/$documentId",
          params: { documentId: member.doc_id },
        });
      })
      .catch((e: Error) => {
        setError(e.message ?? "Failed to accept invite");
      });
  }, [hydrated, accessToken, token, navigate]);

  if (error) {
    return (
      <main className="flex h-screen items-center justify-center p-4">
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle className="text-lg">Invite failed</CardTitle>
            <CardDescription>{error}</CardDescription>
          </CardHeader>
        </Card>
      </main>
    );
  }

  return (
    <main className="flex h-screen items-center justify-center p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-lg">Accepting invite...</CardTitle>
          <CardDescription>Please wait.</CardDescription>
        </CardHeader>
      </Card>
    </main>
  );
}
