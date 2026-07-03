import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { createServerFn } from "@tanstack/react-start";
import { useEffect, useState } from "react";
import { AuthLayout } from "#/features/auth/AuthLayout";
import {
  Card,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

interface ApiError {
  detail: string;
}

type VerifyResult = { success: true } | { success: false; error: string };

const verifyEmail = createServerFn({ method: "POST" })
  .validator((token: string) => token)
  .handler(async ({ data: token }): Promise<VerifyResult> => {
    try {
      const res = await fetch(`${API_BASE}/auth/verify-email`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token }),
      });
      const data = await res.json();

      if (!res.ok) {
        return {
          success: false,
          error: (data as ApiError).detail ?? "Verification failed",
        };
      }

      return { success: true };
    } catch {
      return { success: false, error: "Network error. Please try again." };
    }
  });

export const Route = createFileRoute("/verify-email")({
  loader: ({ location }): Promise<VerifyResult> => {
    const search = location.search as Record<string, string>;
    const token = search.token;

    return token
      ? verifyEmail({ data: token })
      : Promise.resolve({ success: false, error: "No verification token provided." });
  },
  pendingComponent: VerifyEmailPending,
  component: VerifyEmail,
});

function VerifyEmailPending() {
  return (
    <AuthLayout
      eyebrow="Verifying account"
      title="Setting up secure access to your drafts."
      description="Drafthouse verifies email ownership before opening private collaborative documents."
    >
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-lg">Verifying your email...</CardTitle>
          <CardDescription>
            Please wait while we verify your email address.
          </CardDescription>
        </CardHeader>
      </Card>
    </AuthLayout>
  );
}

function VerifyEmail() {
  const result = Route.useLoaderData();
  const navigate = useNavigate();
  const [verifiedEmail] = useState(() => {
    if (!result.success || typeof window === "undefined") return "";
    const pendingEmail = localStorage.getItem("dh_pending_verification_email") ?? "";
    localStorage.removeItem("dh_pending_verification_email");
    return pendingEmail;
  });

  useEffect(() => {
    if (!result.success) return;
    const timer = window.setTimeout(() => {
      navigate({ to: "/login", search: { verified: "1", email: verifiedEmail } });
    }, 2500);
    return () => window.clearTimeout(timer);
  }, [result.success, verifiedEmail, navigate]);

  if (result.success) {
    return (
      <AuthLayout
        eyebrow="Email verified"
        title="Your Drafthouse workspace is ready."
        description="Sign in to create your welcome document and start collaborating in Markdown."
      >
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle className="text-lg">Email verified</CardTitle>
            <CardDescription>
              Your email has been verified successfully. You can now sign in.
            </CardDescription>
          </CardHeader>
          <CardFooter>
            <Link
              to="/login"
              search={{ verified: "1", email: verifiedEmail }}
              className="inline-flex h-7 w-full items-center justify-center rounded-lg bg-primary px-2.5 text-[0.8rem] font-medium text-primary-foreground shadow-sm shadow-primary/25 transition-all hover:-translate-y-0.5 hover:bg-primary/90"
            >
              Continue to Drafthouse
            </Link>
          </CardFooter>
        </Card>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      eyebrow="Verification failed"
      title="The link did not open your workspace."
      description="Verification links can expire. Request a new email to continue creating your Drafthouse account."
    >
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-lg">Verification failed</CardTitle>
          <CardDescription>{result.error}</CardDescription>
        </CardHeader>
        <CardFooter>
          <Link
            to="/resend-verification"
            className="text-xs font-medium text-foreground underline-offset-4 hover:underline"
          >
            Request new verification email
          </Link>
        </CardFooter>
      </Card>
    </AuthLayout>
  );
}
