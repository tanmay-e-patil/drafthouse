import { Link } from '@tanstack/react-router'
import { buttonVariants } from '#/components/ui/buttonVariants'

function AppErrorPage() {
  return (
    <main className="flex min-h-screen items-center justify-center p-8">
      <div className="max-w-sm text-center">
        <h1 className="text-xl font-semibold tracking-tight">Something went wrong</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          The page could not be loaded. Return to your dashboard and try again.
        </p>
        <Link className={buttonVariants({ className: 'mt-6' })} to="/">
          Back to dashboard
        </Link>
      </div>
    </main>
  )
}

function NotFoundPage() {
  return (
    <main className="flex min-h-screen items-center justify-center p-8">
      <div className="max-w-sm text-center">
        <h1 className="text-xl font-semibold tracking-tight">Page not found</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          The page you're looking for doesn't exist.
        </p>
      </div>
    </main>
  )
}

export { AppErrorPage, NotFoundPage }
