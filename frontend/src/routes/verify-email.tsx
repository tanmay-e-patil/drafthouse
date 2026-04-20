import { createFileRoute, useSearch } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { Link } from "@tanstack/react-router";

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

    fetch(`${API_BASE}/auth/verify`, {
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
      <main className="auth-page">
        <div className="auth-card">
          <h1>Verifying your email...</h1>
          <p>Please wait while we verify your email address.</p>
        </div>
      </main>
    );
  }

  if (success) {
    return (
      <main className="auth-page">
        <div className="auth-card">
          <h1>Email verified</h1>
          <p>Your email has been verified successfully. You can now sign in.</p>
          <div className="success-msg">
            Verification complete! You can now log in.
          </div>
          <Link to="/login" className="auth-link">
            Sign in to your account
          </Link>
        </div>
      </main>
    );
  }

  return (
    <main className="auth-page">
      <div className="auth-card">
        <h1>Verification failed</h1>
        {error && <div className="error-msg">{error}</div>}
        <p>
          The verification link may have expired or is invalid. Request a new
          one.
        </p>
        <Link to="/resend-verification" className="auth-link">
          Request new verification email
        </Link>
      </div>
    </main>
  );
}
