import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { FileText, Lock, Radio, ShieldCheck } from "lucide-react";

type AuthLayoutProps = {
  children: ReactNode;
  eyebrow?: string;
  title?: string;
  description?: string;
};

export function AuthLayout({
  children,
  eyebrow = "Private collaborative workspace",
  title = "Write together without losing the thread.",
  description = "Drafthouse keeps markdown drafts, live cursors, sharing controls, and resilient CRDT sync in one focused room.",
}: AuthLayoutProps) {
  return (
    <main className="relative grid min-h-screen overflow-hidden bg-background text-foreground lg:grid-cols-[1.05fr_0.95fr]">
      <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_15%_18%,oklch(0.82_0.12_72_/_0.48),transparent_32%),radial-gradient(circle_at_80%_8%,oklch(0.8_0.09_138_/_0.32),transparent_30%),linear-gradient(135deg,oklch(0.99_0.014_88_/_0.72),oklch(0.94_0.035_91_/_0.86))] dark:bg-[radial-gradient(circle_at_15%_18%,oklch(0.5_0.13_72_/_0.24),transparent_32%),radial-gradient(circle_at_80%_8%,oklch(0.36_0.09_138_/_0.22),transparent_30%),linear-gradient(135deg,oklch(0.2_0.035_74_/_0.88),oklch(0.155_0.03_73_/_0.96))]" />

      <section className="hidden border-r border-border/80 px-10 py-8 lg:flex lg:flex-col">
        <Link to="/" className="flex items-center gap-2 font-semibold tracking-tight">
          <span className="brand-mark flex size-8 items-center justify-center rounded-xl">
            <FileText className="size-4" />
          </span>
          Drafthouse
        </Link>

        <div className="flex flex-1 items-center">
          <div className="max-w-xl">
            <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-primary/20 bg-card/70 px-3 py-1 text-xs font-medium text-muted-foreground shadow-sm backdrop-blur">
              <Radio className="size-3.5 text-accent-foreground" />
              {eyebrow}
            </div>
            <h1 className="font-heading text-balance text-5xl font-bold tracking-tight">
              {title}
            </h1>
            <p className="mt-5 text-pretty text-lg leading-8 text-muted-foreground">
              {description}
            </p>

            <div className="mt-10 grid gap-3">
              <BrandingPoint
                icon={<ShieldCheck className="size-4" />}
                title="One-time secure sessions"
                description="JWT auth, refresh cookies, and short-lived collaboration tickets protect private drafts."
              />
              <BrandingPoint
                icon={<Lock className="size-4" />}
                title="Verified writers only"
                description="Email verification gates access before a team starts sharing documents."
              />
            </div>
          </div>
        </div>
      </section>

      <section className="flex min-h-screen items-center justify-center px-5 py-10 sm:px-8">
        <div className="w-full max-w-sm">
          <Link to="/" className="mb-8 flex items-center justify-center gap-2 font-semibold tracking-tight lg:hidden">
            <span className="brand-mark flex size-8 items-center justify-center rounded-xl">
              <FileText className="size-4" />
            </span>
            Drafthouse
          </Link>
          {children}
        </div>
      </section>
    </main>
  );
}

function BrandingPoint({
  icon,
  title,
  description,
}: {
  icon: ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="flex gap-4 rounded-2xl border border-border/80 bg-card/70 p-4 shadow-sm backdrop-blur transition-all duration-300 hover:-translate-y-1 hover:border-primary/35">
      <div className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm shadow-primary/25">
        {icon}
      </div>
      <div>
        <h2 className="font-heading text-sm font-semibold tracking-tight">{title}</h2>
        <p className="mt-1 text-sm leading-6 text-muted-foreground">{description}</p>
      </div>
    </div>
  );
}
