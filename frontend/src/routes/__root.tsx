import { HeadContent, Outlet, Scripts, createRootRoute } from '@tanstack/react-router'
import { Toaster } from '#/components/ui/sonner'
import { TooltipProvider } from '#/components/ui/tooltip'
import { ThemeProvider } from 'next-themes'

import appCss from '../styles.css?url'

const THEME_INIT_SCRIPT = `(function(){try{var s=window.localStorage.getItem('theme');if(s==='light'||s==='dark'){document.documentElement.classList.add(s);document.documentElement.style.colorScheme=s}else{document.documentElement.style.colorScheme='light'}}catch(e){}})();`

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
