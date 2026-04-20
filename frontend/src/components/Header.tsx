import { Link } from '@tanstack/react-router'
import ThemeToggle from './ThemeToggle'
import { useAuthStore } from '#/features/auth/store'
import { logoutApi } from '#/features/auth/api'
import { useNavigate } from '@tanstack/react-router'

export default function Header() {
  const accessToken = useAuthStore((s) => s.accessToken);
  const clearAuth = useAuthStore((s) => s.clearAuth);
  const navigate = useNavigate();

  async function handleLogout() {
    try {
      await logoutApi();
    } catch {
    }
    clearAuth();
    navigate({ to: '/' });
  }

  return (
    <header className="header">
      <nav className="header-nav">
        <Link to="/" className="logo">
          Drafthouse
        </Link>

        <div className="header-right">
          <ThemeToggle />
          {accessToken ? (
            <button onClick={handleLogout} className="logout-btn">
              Sign out
            </button>
          ) : (
            <>
              <Link to="/login" className="header-link">
                Sign in
              </Link>
              <Link to="/register" className="header-link">
                Sign up
              </Link>
            </>
          )}
        </div>
      </nav>
    </header>
  )
}
