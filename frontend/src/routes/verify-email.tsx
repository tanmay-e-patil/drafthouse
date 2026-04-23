import { createFileRoute, useSearch } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { Link } from "@tanstack/react-router";
import {
  Card,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";

export const Route = createFileRoute("/verify-email")({ component: VerifyEmail });

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

interface ApiError {
  detail: string;
}

function VerifyEmail() {
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [success, setSuccess] = useState(false);
  const search = useSearch({ strict: false });
  const token =
    typeof search === "object" && search !== null && "token" in search
      ? (search as Record<string, string>).token
      : null;

  useEffect(() => {
    if (!token) {
      setError("No verification token provided.");
      setLoading(false);
      return;
    }

    fetch(`${API_BASE}/auth/verify-email`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token }),
    })
      .then(async (res) => {
        const data = await res.json();
        if (!res.ok) {
          const err = data as ApiError;
          setError(err.detail ?? "Verification failed");
        } else {
          setSuccess(true);
        }
      })
      .catch(() => {
        setError("Network error. Please try again.");
      })
      .finally(() => {
        setLoading(false);
      });
  }, [token]);

  if (loading) {
    return (
      <main className="flex h-screen items-center justify-center p-4">
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle className="text-lg">Verifying your email...</CardTitle>
            <CardDescription>
              Please wait while we verify your email address.
            </CardDescription>
          </CardHeader>
        </Card>
      </main>
    );
  }

  if (success) {
    return (
      <main className="flex h-screen items-center justify-center p-4">
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
              className="text-xs font-medium text-foreground underline-offset-4 hover:underline"
            >
              Sign in to your account
            </Link>
          </CardFooter>
        </Card>
      </main>
    );
  }

  return (
    <main className="flex h-screen items-center justify-center p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-lg">Verification failed</CardTitle>
          <CardDescription>
            {error ??
              "The verification link may have expired or is invalid."}
          </CardDescription>
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
    </main>
  );
}
