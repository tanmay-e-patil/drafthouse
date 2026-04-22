import { createFileRoute, Link, useNavigate, useSearch } from "@tanstack/react-router";
import { useState, type FormEvent } from "react";
import { loginApi } from "#/features/auth/api";
import { useAuthStore } from "#/features/auth/store";

export const Route = createFileRoute("/login")({ component: Login });

function Login() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const setAccessToken = useAuthStore((s) => s.setAccessToken);
  const storeEmail = useAuthStore((s) => s.setEmail);
  const navigate = useNavigate();
  const search = useSearch({ strict: false });
  const redirectTo =
    typeof search === "object" && search !== null && "redirect" in search
      ? (search as Record<string, string>).redirect
      : null;

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const data = await loginApi(email, password);
      setAccessToken(data.access_token);
      storeEmail(email);
      if (redirectTo) {
        navigate({ to: redirectTo });
      } else if (data.welcome_doc_id) {
        navigate({
          to: "/documents/$documentId",
          params: { documentId: data.welcome_doc_id },
        });
      } else {
        navigate({ to: "/" });
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="auth-page">
      <div className="auth-card">
        <h1>Sign in</h1>

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

          <div className="field">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Your password"
            />
          </div>

          <button type="submit" className="submit-btn" disabled={loading}>
            {loading ? "Signing in..." : "Sign in"}
          </button>
        </form>

        <Link to="/register" className="auth-link">
          Need an account? Sign up
        </Link>
      </div>
    </main>
  );
}
