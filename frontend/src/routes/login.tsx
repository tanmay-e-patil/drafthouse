import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/login")({ component: Login });

function Login() {
  return (
    <main className="auth-page">
      <div className="auth-card">
        <h1>Sign in</h1>
        <p>Login functionality coming soon.</p>
        <Link to="/register" className="auth-link">
          Need an account? Sign up
        </Link>
      </div>
    </main>
  );
}
