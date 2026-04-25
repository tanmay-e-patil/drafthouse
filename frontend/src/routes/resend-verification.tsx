import { createFileRoute, Link } from "@tanstack/react-router";
import { useState, type FormEvent } from "react";
import { Button } from "#/components/ui/button";
import { Input } from "#/components/ui/input";
import { Label } from "#/components/ui/label";
import { AuthLayout } from "#/features/auth/AuthLayout";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";

export const Route = createFileRoute("/resend-verification")({
  component: ResendVerification,
});

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

interface ApiError {
  detail: string;
}

function ResendVerification() {
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const res = await fetch(`${API_BASE}/auth/resend-verification`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email }),
      });

      const data = await res.json();

      if (!res.ok) {
        const err = data as ApiError;
        setError(err.detail ?? "Request failed");
        return;
      }

      setSuccess(true);
    } catch {
      setError("Network error. Please try again.");
    } finally {
      setLoading(false);
    }
  }

  if (success) {
    return (
      <AuthLayout
        eyebrow="Verification sent"
        title="One more click before your writing room opens."
        description="Verification keeps shared drafts limited to confirmed Drafthouse accounts."
      >
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle className="text-lg">Check your email</CardTitle>
            <CardDescription>
              If an account exists with{" "}
              <strong className="text-foreground">{email}</strong>, a new
              verification email has been sent.
            </CardDescription>
          </CardHeader>
          <CardFooter>
            <Link
              to="/"
              className="text-xs font-medium text-foreground underline-offset-4 hover:underline"
            >
              Back to home
            </Link>
          </CardFooter>
        </Card>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      eyebrow="Verify your account"
      title="Request a fresh verification link."
      description="Drafthouse requires verified email addresses before users can access private documents."
    >
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-lg">Resend verification email</CardTitle>
          <CardDescription>
            Enter your email to receive a new verification link
          </CardDescription>
        </CardHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <CardContent className="space-y-3">
            {error && (
              <p className="rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {error}
              </p>
            )}
            <div className="space-y-1.5">
              <Label htmlFor="email" className="text-xs">
                Email
              </Label>
              <Input
                id="email"
                type="email"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                className="h-8 text-sm"
              />
            </div>
          </CardContent>
          <CardFooter className="flex-col gap-3">
            <Button
              type="submit"
              className="w-full"
              size="sm"
              disabled={loading}
            >
              {loading ? "Sending..." : "Send verification email"}
            </Button>
            <Link
              to="/register"
              className="text-xs text-muted-foreground underline-offset-4 hover:underline"
            >
              Need an account? Sign up
            </Link>
          </CardFooter>
        </form>
      </Card>
    </AuthLayout>
  );
}
