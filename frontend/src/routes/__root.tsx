import { HeadContent, Link, Outlet, Scripts, createRootRoute } from '@tanstack/react-router'
import { buttonVariants } from '#/components/ui/button'
import { Toaster } from '#/components/ui/sonner'
import { TooltipProvider } from '#/components/ui/tooltip'
import { ThemeProvider } from 'next-themes'

import appCss from '../styles.css?url'

const THEME_INIT_SCRIPT = `(function(){try{var s=window.localStorage.getItem('theme');var p=window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';var t=s==='light'||s==='dark'?s:p;document.documentElement.classList.add(t);document.documentElement.style.colorScheme=t}catch(e){}})();`

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'Drafthouse' },
    ],
    links: [{ rel: 'stylesheet', href: appCss }],
  }),
  component: RootLayout,
  errorComponent: AppErrorPage,
})

function RootLayout() {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
        <HeadContent />
      </head>
      <body className="font-sans antialiased">
        <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
          <TooltipProvider delay={300}>
            <Outlet />
          </TooltipProvider>
        </ThemeProvider>
        <Toaster richColors position="bottom-right" />
        <Scripts />
      </body>
    </html>
  )
}

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
