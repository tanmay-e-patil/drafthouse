import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState, useCallback, type ReactNode } from "react";
import { createDocumentApi } from "#/features/documents/api";
import { useDocumentStore } from "#/features/documents/store";
import { useAuthStore } from "#/features/auth/store";
import Sidebar from "#/components/Sidebar";
import { Button } from "#/components/ui/button";
import { CommandPalette } from "#/features/documents/CommandPalette";
import { useDocumentHotkeys } from "#/features/documents/useDocumentHotkeys";
import {
  ArrowRight,
  Check,
  FileText,
  GitBranch,
  Lock,
  Plus,
  Radio,
  ShieldCheck,
  Sparkles,
  Users,
  Zap,
} from "lucide-react";
import { Link } from "@tanstack/react-router";
import { notifyTransientError } from "#/shared/errors";

export const Route = createFileRoute("/")({ component: Dashboard });

function Dashboard() {
  const navigate = useNavigate();
  const accessToken = useAuthStore((s) => s.accessToken);
  const hydrated = useAuthStore((s) => s.hydrated);
  const hydrate = useAuthStore((s) => s.hydrate);
  const { prependDocument } = useDocumentStore();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  const toggleSidebar = useCallback(() => setSidebarCollapsed((v) => !v), []);
  const openPalette = useCallback(() => setPaletteOpen(true), []);

  useDocumentHotkeys({
    onOpenPalette: openPalette,
    onToggleSidebar: toggleSidebar,
  });

  if (!hydrated) return null;

  if (!accessToken) {
    return <LandingPage />;
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
      <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={toggleSidebar} />
      <main className="flex flex-1 flex-col overflow-hidden">
        <div className="flex h-12 items-center justify-between border-b border-border px-4">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <FileText className="size-4" />
            <span>Documents</span>
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="gap-1.5 text-muted-foreground"
            onClick={() => {
              createDocumentApi().then((doc) => {
                prependDocument(doc);
                navigate({
                  to: "/documents/$documentId",
                  params: { documentId: doc.id },
                });
              }).catch((error) => notifyTransientError(error));
            }}
          >
            <Plus className="size-3.5" />
            New
          </Button>
        </div>
        <div className="flex flex-1 items-center justify-center p-8">
          <div className="text-center text-muted-foreground">
            <FileText className="mx-auto mb-3 size-10 opacity-30" />
            <p className="text-sm">Select a document or create a new one</p>
            <p className="mt-1 text-xs text-muted-foreground/60">
              Press <kbd className="rounded border border-border bg-muted px-1 py-0.5 text-[10px] font-mono">⌘ K</kbd> to search your documents
            </p>
          </div>
        </div>
      </main>
    </div>
  );
}

function LandingPage() {
  return (
    <main className="min-h-screen overflow-hidden bg-background text-foreground">
      <section className="relative isolate border-b border-border/80">
        <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_18%_14%,oklch(0.82_0.12_72_/_0.5),transparent_34%),radial-gradient(circle_at_84%_6%,oklch(0.81_0.09_138_/_0.36),transparent_32%),linear-gradient(180deg,oklch(0.99_0.015_88_/_0.65),oklch(0.95_0.03_90_/_0.85))] dark:bg-[radial-gradient(circle_at_18%_14%,oklch(0.52_0.13_72_/_0.28),transparent_34%),radial-gradient(circle_at_84%_6%,oklch(0.38_0.09_138_/_0.24),transparent_32%),linear-gradient(180deg,oklch(0.2_0.035_74_/_0.78),oklch(0.16_0.03_73_/_0.95))]" />
        <header className="mx-auto flex max-w-7xl items-center justify-between px-5 py-5 sm:px-8">
          <Link to="/" className="flex items-center gap-2 font-semibold tracking-tight">
            <span className="brand-mark flex size-8 items-center justify-center rounded-xl">
              <FileText className="size-4" />
            </span>
            Drafthouse
          </Link>
          <nav className="flex items-center gap-2">
            <Button variant="ghost" size="sm" nativeButton={false} render={<Link to="/login" />}>
              Sign in
            </Button>
            <Button size="sm" nativeButton={false} render={<Link to="/register" />}>
              Start writing
            </Button>
          </nav>
        </header>

        <div className="mx-auto grid max-w-7xl items-center gap-12 px-5 pb-20 pt-10 sm:px-8 lg:grid-cols-[1fr_0.92fr] lg:pb-28 lg:pt-16">
          <div className="max-w-3xl">
            <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-primary/20 bg-card/70 px-3 py-1 text-xs font-medium text-muted-foreground shadow-sm backdrop-blur">
              <Radio className="size-3.5 text-accent-foreground" />
              Real-time markdown collaboration
            </div>
            <h1 className="font-heading text-balance text-5xl font-bold tracking-tight sm:text-6xl lg:text-7xl">
              A focused writing room for teams that think in Markdown.
            </h1>
            <p className="mt-6 max-w-2xl text-pretty text-lg leading-8 text-muted-foreground">
              Drafthouse gives your team live cursors, resilient CRDT sync, sharing controls, and a clean editor built for product notes, specs, research, and long-form work.
            </p>
            <div className="mt-8 flex flex-col gap-3 sm:flex-row">
              <Button size="lg" nativeButton={false} render={<Link to="/register" />}>
                Create your first draft
                <ArrowRight className="size-4" />
              </Button>
              <Button variant="outline" size="lg" nativeButton={false} render={<Link to="/login" />}>
                Sign in
              </Button>
            </div>
            <dl className="mt-10 grid max-w-xl grid-cols-3 gap-4 text-sm">
              <Metric value="100" label="editors per doc" />
              <Metric value="100ms" label="WAL buffer" />
              <Metric value="1MB" label="doc cap" />
            </dl>
          </div>

          <HeroEditorMockup />
        </div>
      </section>

      <section className="mx-auto max-w-7xl px-5 py-20 sm:px-8">
        <div className="max-w-2xl">
          <p className="text-sm font-medium text-primary">Built for collaborative docs</p>
          <h2 className="mt-3 font-heading text-3xl font-bold tracking-tight sm:text-4xl">
            Everything needed for a dependable shared writing space.
          </h2>
        </div>
        <div className="mt-10 grid gap-4 md:grid-cols-3">
          <FeatureCard
            icon={<Users className="size-5" />}
            title="Live presence"
            description="See active editors, cursors, idle state, and document title changes as they happen."
          />
          <FeatureCard
            icon={<GitBranch className="size-5" />}
            title="Conflict-free editing"
            description="Yrs CRDT sync merges concurrent edits cleanly across reconnects and offline moments."
          />
          <FeatureCard
            icon={<ShieldCheck className="size-5" />}
            title="Secure sharing"
            description="Invite links, public read-only docs, one-time WebSocket tickets, and owner controls are built in."
          />
        </div>
      </section>

      <section className="border-y border-border/80 bg-muted/55">
        <div className="mx-auto grid max-w-7xl gap-10 px-5 py-20 sm:px-8 lg:grid-cols-[0.85fr_1fr]">
          <div>
            <p className="text-sm font-medium text-primary">Writer-first workflow</p>
            <h2 className="mt-3 font-heading text-3xl font-bold tracking-tight sm:text-4xl">
              Draft, preview, share, and keep moving.
            </h2>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            {[
              "CodeMirror markdown editor with keyboard shortcuts",
              "Preview mode for mobile and polished reading",
              "Command palette for fast document switching",
              "Share modal with roles, expiry, and public access",
              "Dark mode and selectable editor typography",
              "Recoverable snapshots with checksum validation",
            ].map((item) => (
              <div key={item} className="flex gap-3 rounded-2xl border border-border/80 bg-card/80 p-4 text-sm shadow-sm transition-all duration-300 hover:-translate-y-1 hover:border-primary/35 hover:shadow-md">
                <Check className="mt-0.5 size-4 shrink-0 text-accent-foreground" />
                <span>{item}</span>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="mx-auto max-w-7xl px-5 py-20 sm:px-8">
        <div className="ambient-panel rounded-3xl border border-border/80 p-8 shadow-xl shadow-foreground/8 sm:p-10 lg:flex lg:items-center lg:justify-between">
          <div className="max-w-2xl">
            <div className="mb-4 flex items-center gap-2 text-sm font-medium text-muted-foreground">
              <Lock className="size-4" />
              Designed for private team knowledge
            </div>
            <h2 className="font-heading text-3xl font-bold tracking-tight">Start with a blank page. Keep the whole team in sync.</h2>
            <p className="mt-3 text-muted-foreground">
              Create an account, verify your email, and Drafthouse creates a welcome document with shortcuts and examples.
            </p>
          </div>
          <Button size="lg" className="mt-8 lg:mt-0" nativeButton={false} render={<Link to="/register" />}>
            Get started
            <ArrowRight className="size-4" />
          </Button>
        </div>
      </section>
    </main>
  );
}

function HeroEditorMockup() {
  return (
    <div className="relative mx-auto w-full max-w-xl">
      <div className="absolute -left-6 top-10 z-20 hidden rounded-2xl border border-border/80 bg-card/90 p-3 shadow-xl backdrop-blur motion-safe:animate-in motion-safe:fade-in-0 motion-safe:slide-in-from-left-4 sm:block">
        <div className="flex -space-x-2">
          {['AM', 'RK', 'JS'].map((initials, index) => (
            <span
              key={initials}
              className="flex size-8 items-center justify-center rounded-full border-2 border-background text-[10px] font-semibold text-white"
              style={{ backgroundColor: ['#2563eb', '#16a34a', '#9333ea'][index] }}
            >
              {initials}
            </span>
          ))}
        </div>
        <p className="mt-2 text-xs text-muted-foreground">3 editors active</p>
      </div>
      <div className="rounded-[2rem] border border-border/80 bg-card/70 p-3 shadow-2xl shadow-primary/15 backdrop-blur">
        <div className="overflow-hidden rounded-[1.4rem] border border-border/80 bg-card">
          <div className="flex items-center justify-between border-b border-border/80 px-4 py-3">
            <div className="flex items-center gap-2">
              <span className="size-2.5 rounded-full bg-red-400" />
              <span className="size-2.5 rounded-full bg-amber-400" />
              <span className="size-2.5 rounded-full bg-emerald-400" />
            </div>
            <div className="flex items-center gap-2 rounded-full bg-accent px-2.5 py-1 text-xs font-medium text-accent-foreground">
              <Zap className="size-3" />
              synced
            </div>
          </div>
          <div className="grid min-h-[430px] md:grid-cols-[0.38fr_1fr]">
            <aside className="hidden border-r border-border/80 bg-muted/55 p-4 md:block">
              <p className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Docs</p>
              {['Product Roadmap', 'Launch Notes', 'Research'].map((doc, index) => (
                <div key={doc} className={`mt-3 rounded-xl p-3 text-xs transition-all duration-300 ${index === 0 ? 'bg-card shadow-sm' : 'text-muted-foreground'}`}>
                  <p className="font-medium text-foreground">{doc}</p>
                  <p className="mt-1 text-muted-foreground">{index === 0 ? '2 min ago' : 'Yesterday'}</p>
                </div>
              ))}
            </aside>
            <div className="p-5">
              <div className="mb-5 flex flex-wrap items-center gap-2 border-b border-border pb-3 text-xs text-muted-foreground">
                <span className="rounded-md bg-muted px-2 py-1">H1</span>
                <span className="rounded-md bg-muted px-2 py-1">B</span>
                <span className="rounded-md bg-muted px-2 py-1">I</span>
                <span className="ml-auto rounded-md bg-primary px-2 py-1 text-primary-foreground shadow-sm">Preview</span>
              </div>
              <article className="prose prose-sm max-w-none dark:prose-invert prose-headings:font-heading">
                <h2>Q3 product roadmap</h2>
                <p>
                  Align on launch scope, edge cases, and owner notes before the customer beta.
                </p>
                <ul>
                  <li>Finalize invite flow permissions</li>
                  <li>Stress test reconnect and replay</li>
                  <li>Publish onboarding doc</li>
                </ul>
              </article>
              <div className="mt-8 rounded-2xl border border-dashed border-primary/35 bg-secondary/60 p-4">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Sparkles className="size-4 text-amber-500" />
                  Changes save continuously
                </div>
                <p className="mt-2 text-xs leading-5 text-muted-foreground">
                  Snapshots, WAL replay, and CRDT merges keep every draft recoverable after reconnects.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function Metric({ value, label }: { value: string; label: string }) {
  return (
    <div className="rounded-2xl border border-border/80 bg-card/70 p-4 shadow-sm backdrop-blur transition-all duration-300 hover:-translate-y-1 hover:border-primary/35">
      <dt className="text-2xl font-semibold tracking-tight">{value}</dt>
      <dd className="mt-1 text-xs text-muted-foreground">{label}</dd>
    </div>
  );
}

function FeatureCard({
  icon,
  title,
  description,
}: {
  icon: ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="ambient-panel rounded-3xl border border-border/80 p-6 shadow-sm transition-all duration-300 hover:-translate-y-1 hover:border-primary/35 hover:shadow-lg">
      <div className="mb-5 flex size-11 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-sm shadow-primary/25">
        {icon}
      </div>
      <h3 className="font-heading text-lg font-semibold tracking-tight">{title}</h3>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{description}</p>
    </div>
  );
}
