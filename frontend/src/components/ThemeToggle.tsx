import { useTheme } from 'next-themes'
import { Moon, Sun, Monitor } from 'lucide-react'
import { Button } from '#/components/ui/button'

function ThemeIcon({ theme }: { theme: string | undefined }) {
  if (theme === 'dark') return <Moon className="size-4" />
  if (theme === 'light') return <Sun className="size-4" />
  return <Monitor className="size-4" />
}

export default function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const cycle = () => {
    if (theme === 'light') setTheme('dark')
    else if (theme === 'dark') setTheme('system')
    else setTheme('light')
  }

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={cycle}
      aria-label={`Current: ${theme ?? 'system'}. Click to switch.`}
      className="size-8 text-muted-foreground hover:text-accent-foreground"
    >
      <ThemeIcon theme={theme} />
    </Button>
  )
}
