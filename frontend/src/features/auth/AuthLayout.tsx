import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { Check, FileText, Radio, ShieldCheck, Users, Zap } from "lucide-react";

type AuthLayoutProps = {
  children: ReactNode;
  eyebrow?: string;
  title?: string;
  description?: string;
};

export function AuthLayout({
  children,
  eyebrow = "Private collaborative workspace",
  title = "Get back to the draft everyone trusts.",
  description = "Sign in to keep specs, launch notes, and product decisions moving in one focused Markdown room.",
}: AuthLayoutProps) {
  return (
    <main className="relative grid min-h-screen overflow-hidden bg-background text-foreground lg:grid-cols-[1.08fr_0.92fr]">
      <div className="absolute inset-0 -z-20 bg-[radial-gradient(circle_at_15%_18%,oklch(0.82_0.12_72_/_0.48),transparent_32rem),radial-gradient(circle_at_80%_8%,oklch(0.8_0.09_138_/_0.32),transparent_30rem),linear-gradient(135deg,var(--background),color-mix(in_oklch,var(--background)_78%,var(--secondary)))]" />
      <div className="pointer-events-none absolute inset-y-0 left-1/2 -z-10 w-screen max-w-7xl -translate-x-1/2 border-x border-border/50 [background-image:linear-gradient(to_right,color-mix(in_oklch,var(--border)_45%,transparent)_1px,transparent_1px)] [background-size:25%_100%]" />

      <section className="hidden border-r border-border/70 px-10 py-8 lg:flex lg:flex-col">
        <Link to="/" className="flex items-center gap-2 font-semibold tracking-tight">
          <span className="brand-mark flex size-8 items-center justify-center rounded-xl">
            <FileText className="size-4" />
          </span>
          Drafthouse
        </Link>

        <div className="flex flex-1 items-center">
          <div className="w-full max-w-2xl">
            <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-primary/20 bg-card/70 px-3 py-1 text-xs font-medium text-muted-foreground shadow-sm backdrop-blur">
              <Radio className="size-3.5 text-primary" />
              {eyebrow}
            </div>
            <h1 className="font-heading text-balance text-5xl font-bold tracking-tight xl:text-6xl">
              {title}
            </h1>
            <p className="mt-5 max-w-xl text-pretty text-lg leading-8 text-muted-foreground">
              {description}
            </p>

            <div className="mt-10 grid gap-4 xl:grid-cols-[0.85fr_1fr]">
              <ProductFrame />
              <div className="grid gap-3">
                <BrandingPoint icon={<Users className="size-4" />} title="Live team presence" description="See who is editing before you change the plan." />
                <BrandingPoint icon={<ShieldCheck className="size-4" />} title="Safe sharing" description="Verified accounts and owner controls protect private drafts." />
                <BrandingPoint icon={<Zap className="size-4" />} title="Always in sync" description="Reconnects merge work without overwriting teammates." />
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="flex min-h-screen items-center justify-center px-5 py-10 sm:px-8">
        <div className="w-full max-w-md">
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

function ProductFrame() {
  return (
    <div className="ambient-panel group rounded-[2rem] border border-border/80 p-3 shadow-xl shadow-primary/10 transition hover:-translate-y-1">
      <div className="overflow-hidden rounded-[1.35rem] border border-border/80 bg-card">
        <div className="flex items-center justify-between border-b border-border/80 px-3 py-2">
          <div className="flex gap-1.5"><span className="size-2 rounded-full bg-red-400" /><span className="size-2 rounded-full bg-amber-400" /><span className="size-2 rounded-full bg-emerald-400" /></div>
          <span className="rounded-full bg-accent px-2 py-0.5 text-[10px] font-medium text-accent-foreground">synced</span>
        </div>
        <div className="p-4">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">Spec draft</p>
          <h2 className="mt-3 font-heading text-2xl font-bold tracking-tight">Beta launch plan</h2>
          <div className="mt-4 space-y-2 text-sm text-muted-foreground">
            {['Invite flow approved', 'Reconnect test running', 'Docs owner assigned'].map((item) => (
              <div key={item} className="flex items-center gap-2 rounded-xl bg-muted/55 p-2"><Check className="size-3.5 text-primary" />{item}</div>
            ))}
          </div>
          <div className="mt-5 flex -space-x-2">
            {['AM', 'RK', 'JS'].map((x, i) => <span key={x} className="grid size-8 place-items-center rounded-full border-2 border-card text-[10px] font-semibold text-white" style={{ backgroundColor: ['#2563eb', '#16a34a', '#9333ea'][i] }}>{x}</span>)}
          </div>
        </div>
      </div>
    </div>
  );
}

function BrandingPoint({ icon, title, description }: { icon: ReactNode; title: string; description: string }) {
  return (
    <div className="group flex gap-4 rounded-2xl border border-border/80 bg-card/70 p-4 shadow-sm backdrop-blur transition duration-300 hover:-translate-y-1 hover:border-primary/35 hover:shadow-lg">
      <div className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm shadow-primary/25 transition group-hover:scale-105">
        {icon}
      </div>
      <div>
        <h2 className="font-heading text-sm font-semibold tracking-tight">{title}</h2>
        <p className="mt-1 text-sm leading-6 text-muted-foreground">{description}</p>
      </div>
    </div>
  );
}
