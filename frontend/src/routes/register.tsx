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

export const Route = createFileRoute("/register")({ component: Register });

const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

interface ApiError {
  detail: string;
}

function Register() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);

    if (password !== confirmPassword) {
      setError("Passwords do not match");
      return;
    }

    setLoading(true);

    try {
      const res = await fetch(`${API_BASE}/auth/register`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, password }),
      });

      const data = await res.json();

      if (!res.ok) {
        const err = data as ApiError;
        setError(err.detail ?? "Registration failed");
        return;
      }

      localStorage.setItem("dh_pending_verification_email", email);
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
        eyebrow="Welcome to Drafthouse"
        title="Confirm once. Then write with the team."
        description="Verify your email to unlock private Markdown rooms, live collaboration, and safe sharing."
      >
        <Card className="w-full overflow-hidden rounded-[2rem] border-border/80 bg-card/80 shadow-2xl shadow-primary/10 backdrop-blur">
          <CardHeader className="space-y-3 pb-4">
            <div className="inline-flex w-fit items-center rounded-full border border-primary/20 bg-primary/10 px-3 py-1 text-xs font-medium text-primary">
              Almost there
            </div>
            <CardTitle className="font-heading text-3xl tracking-tight">Check your email.</CardTitle>
            <CardDescription>
              We sent a verification link to{" "}
              <strong className="text-foreground">{email}</strong>. Please
              click the link to verify your account before logging in.
            </CardDescription>
          </CardHeader>
          <CardFooter className="flex-col gap-3">
            <Link
              to="/resend-verification"
              search={{ email }}
              className="inline-flex h-7 w-full items-center justify-center rounded-lg bg-primary px-2.5 text-[0.8rem] font-medium text-primary-foreground shadow-sm shadow-primary/25 transition-all hover:-translate-y-0.5 hover:bg-primary/90"
            >
              Resend verification email
            </Link>
            <div className="flex w-full justify-between text-xs text-muted-foreground">
              <Link to="/register" className="underline-offset-4 hover:underline">
                Change email
              </Link>
              <Link to="/login" className="underline-offset-4 hover:underline">
                Sign in
              </Link>
            </div>
            <p className="text-xs text-muted-foreground">
              Didn&apos;t get it? Check spam or request a fresh link.
            </p>
          </CardFooter>
        </Card>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      eyebrow="Start a private writing room"
      title="Create the draft your team can trust."
      description="Start a private Markdown room for specs, launches, research, and decisions — without a bloated workspace."
    >
      <Card className="w-full overflow-hidden rounded-[2rem] border-border/80 bg-card/80 shadow-2xl shadow-primary/10 backdrop-blur">
        <CardHeader className="space-y-3 pb-4">
          <div className="inline-flex w-fit items-center rounded-full border border-primary/20 bg-primary/10 px-3 py-1 text-xs font-medium text-primary">
            First shared draft
          </div>
          <CardTitle className="font-heading text-3xl tracking-tight">Start writing in seconds.</CardTitle>
          <CardDescription>
            Create an account, verify your email, and open a focused room for team docs.
          </CardDescription>
        </CardHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <CardContent className="space-y-3">
            {error && (
              <div className="space-y-2 rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
                <p>{error}</p>
                {error.includes("verification email") && (
                  <Link
                    to="/resend-verification"
                    search={{ email }}
                    className="font-medium underline-offset-4 hover:underline"
                  >
                    Request a new verification email
                  </Link>
                )}
              </div>
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
            <div className="space-y-1.5">
              <Label htmlFor="password" className="text-xs">
                Password
              </Label>
              <Input
                id="password"
                type="password"
                required
                minLength={8}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="At least 8 characters"
                className="h-8 text-sm"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="confirm-password" className="text-xs">
                Confirm password
              </Label>
              <Input
                id="confirm-password"
                type="password"
                required
                minLength={8}
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Repeat your password"
                className="h-8 text-sm"
              />
            </div>
          </CardContent>
          <CardFooter className="flex-col gap-3 pt-2">
            <Button
              type="submit"
              className="h-10 w-full"
              disabled={loading}
            >
              {loading ? "Creating account..." : "Start writing"}
            </Button>
            <p className="text-xs text-muted-foreground">
              Already have an account?{" "}
              <Link
                to="/login"
                className="font-medium text-foreground underline-offset-4 hover:underline"
              >
                Sign in
              </Link>
            </p>
          </CardFooter>
        </form>
      </Card>
    </AuthLayout>
  );
}
