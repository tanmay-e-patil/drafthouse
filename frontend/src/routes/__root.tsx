import { HeadContent, Link, Outlet, Scripts, createRootRoute } from '@tanstack/react-router'
import { buttonVariants } from '#/components/ui/button'
import { lazy, Suspense } from 'react'
import { TooltipProvider } from '#/components/ui/tooltip'
import { ThemeProvider } from 'next-themes'

import appCss from '../styles.css?url'

const LazyToaster = lazy(() =>
  import('#/components/ui/sonner').then((m) => ({ default: m.Toaster }))
)

const THEME_INIT_SCRIPT = `(function(){try{var s=window.localStorage.getItem('theme');var p=window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';var t=s==='light'||s==='dark'?s:p;document.documentElement.classList.add(t);document.documentElement.style.colorScheme=t}catch(e){}})();`
const SITE_DESCRIPTION =
  'Drafthouse is a focused collaborative Markdown editor with live cursors, secure sharing, and resilient team document sync.'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'Drafthouse | Collaborative Markdown Editor for Teams' },
      { name: 'description', content: SITE_DESCRIPTION },
      { name: 'application-name', content: 'Drafthouse' },
      { name: 'robots', content: 'index, follow' },
      { name: 'theme-color', content: '#f5f0df' },
      { property: 'og:type', content: 'website' },
      { property: 'og:site_name', content: 'Drafthouse' },
      { property: 'og:title', content: 'Drafthouse | Collaborative Markdown Editor for Teams' },
      { property: 'og:description', content: SITE_DESCRIPTION },
      { property: 'og:image', content: '/logo512.png' },
      { name: 'twitter:card', content: 'summary_large_image' },
      { name: 'twitter:title', content: 'Drafthouse | Collaborative Markdown Editor for Teams' },
      { name: 'twitter:description', content: SITE_DESCRIPTION },
      { name: 'twitter:image', content: '/logo512.png' },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', href: '/favicon.svg', type: 'image/svg+xml' },
      { rel: 'alternate icon', href: '/favicon.ico' },
      { rel: 'apple-touch-icon', href: '/logo192.png' },
      { rel: 'manifest', href: '/manifest.json' },
    ],
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
        <Suspense fallback={null}>
          <LazyToaster richColors position="bottom-right" />
        </Suspense>
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
