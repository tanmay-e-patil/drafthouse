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
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
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
        title="One verification step before your first shared draft."
        description="Confirm your email to unlock private markdown workspaces, live collaboration, and secure sharing."
      >
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle className="text-lg">Check your email</CardTitle>
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
      title="Create a home for notes, specs, and product thinking."
      description="Your first verified login creates a welcome document with shortcuts and examples so you can start drafting immediately."
    >
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-lg">Create your account</CardTitle>
          <CardDescription>
            Sign up to start writing with Drafthouse
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
          </CardContent>
          <CardFooter className="flex-col gap-3">
            <Button
              type="submit"
              className="w-full"
              size="sm"
              disabled={loading}
            >
              {loading ? "Creating account..." : "Create account"}
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
