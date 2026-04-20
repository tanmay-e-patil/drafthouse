import { createFileRoute, Link } from "@tanstack/react-router";
import { useState, type FormEvent } from "react";

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
      <main className="auth-page">
        <div className="auth-card">
          <h1>Check your email</h1>
          <p>
            If an account exists with <strong>{email}</strong>, a new
            verification email has been sent.
          </p>
          <div className="success-msg">
            Verification email sent! Check your inbox.
          </div>
          <Link to="/" className="auth-link">
            Back to home
          </Link>
        </div>
      </main>
    );
  }

  return (
    <main className="auth-page">
      <div className="auth-card">
        <h1>Resend verification email</h1>
        <p>Enter your email to receive a new verification link.</p>

        {error && <div className="error-msg">{error}</div>}

        <form onSubmit={handleSubmit}>
          <div className="field">
            <label htmlFor="email">Email</label>
            <input
              id="email"
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
            />
          </div>

          <button type="submit" className="submit-btn" disabled={loading}>
            {loading ? "Sending..." : "Send verification email"}
          </button>
        </form>

        <Link to="/register" className="auth-link">
          Need an account? Sign up
        </Link>
      </div>
    </main>
  );
}
