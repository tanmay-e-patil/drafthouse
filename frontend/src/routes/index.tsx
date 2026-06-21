import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
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
  ChevronDown,
  Command,
  FileText,
  GitBranch,
  Layers3,
  Lock,
  MessageSquareText,
  MousePointer2,
  Plus,
  Radio,
  Search,
  ShieldCheck,
  Sparkles,
  Users,
  Zap,
} from "lucide-react";
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
              createDocumentApi()
                .then((doc) => {
                  prependDocument(doc);
                  navigate({
                    to: "/documents/$documentId",
                    params: { documentId: doc.id },
                  });
                })
                .catch((error) => notifyTransientError(error));
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
              Press <kbd className="rounded border border-border bg-muted px-1 py-0.5 font-mono text-[10px]">⌘ K</kbd> to search your documents
            </p>
          </div>
        </div>
      </main>
    </div>
  );
}

const tabs = [
  { name: "Specs", text: "Turn messy planning into one living decision record.", icon: FileText },
  { name: "Launches", text: "Coordinate notes, owners, and approvals before ship day.", icon: Radio },
  { name: "Research", text: "Keep interviews, insights, and next steps searchable.", icon: Search },
];

function LandingPage() {
  const [activeTab, setActiveTab] = useState(0);
  const ActiveIcon = tabs[activeTab].icon;

  return (
    <main className="min-h-screen overflow-hidden bg-background text-foreground">
      <section className="relative isolate border-b border-border/70">
        <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_16%_10%,oklch(0.82_0.12_72_/_0.45),transparent_30rem),radial-gradient(circle_at_84%_4%,oklch(0.78_0.09_140_/_0.35),transparent_28rem),linear-gradient(180deg,var(--background),color-mix(in_oklch,var(--background)_82%,var(--secondary)))]" />
        <GridLines />
        <header className="sticky top-0 z-40 border-b border-border/60 bg-background/70 backdrop-blur-xl">
          <div className="mx-auto flex max-w-7xl items-center justify-between px-5 py-4 sm:px-8">
            <Link to="/" className="flex items-center gap-2 font-semibold tracking-tight">
              <span className="brand-mark flex size-8 items-center justify-center rounded-xl">
                <FileText className="size-4" />
              </span>
              Drafthouse
            </Link>
            <nav className="hidden items-center gap-1 md:flex">
              <MegaMenu />
              <a className="rounded-lg px-3 py-2 text-sm text-muted-foreground transition hover:bg-muted/70 hover:text-foreground" href="#workflow">Workflow</a>
              <a className="rounded-lg px-3 py-2 text-sm text-muted-foreground transition hover:bg-muted/70 hover:text-foreground" href="#security">Security</a>
            </nav>
            <div className="flex items-center gap-2">
              <Button variant="ghost" size="sm" nativeButton={false} render={<Link to="/login" />}>Sign in</Button>
              <Button size="sm" nativeButton={false} render={<Link to="/register" />}>Start writing</Button>
            </div>
          </div>
        </header>

        <div className="mx-auto max-w-7xl px-5 pb-16 pt-14 sm:px-8 lg:pb-24">
          <div className="mx-auto max-w-4xl text-center">
            <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-primary/20 bg-card/70 px-3 py-1 text-xs font-medium text-muted-foreground shadow-sm backdrop-blur">
              <Zap className="size-3.5 text-primary" />
              Product docs that stay aligned while everyone edits
            </div>
            <h1 className="font-heading text-balance text-5xl font-bold tracking-tight sm:text-7xl lg:text-8xl">
              Get the whole team to the same draft faster.
            </h1>
            <p className="mx-auto mt-6 max-w-2xl text-pretty text-lg leading-8 text-muted-foreground">
              Drafthouse gives product and engineering one focused Markdown room for specs, launch notes, and decisions — with live presence and safe sharing built in.
            </p>
            <div className="mt-8 flex flex-col justify-center gap-3 sm:flex-row">
              <Button size="lg" nativeButton={false} render={<Link to="/register" />}>Start writing<ArrowRight className="size-4" /></Button>
              <Button variant="outline" size="lg" nativeButton={false} render={<Link to="/login" />}>Sign in</Button>
            </div>
          </div>

          <div className="relative mt-14">
            <div className="absolute inset-x-8 top-8 h-40 rounded-full bg-primary/20 blur-3xl" />
            <ProductScreenshot activeTab={activeTab} />
          </div>
        </div>
      </section>

      <section id="workflow" className="relative mx-auto max-w-7xl px-5 py-20 sm:px-8">
        <GridLines />
        <div className="grid gap-4 lg:grid-cols-6">
          <Bento className="lg:col-span-3 lg:row-span-2" icon={<Users className="size-5" />} title="See who is shaping the doc" text="Live cursors, active editors, and title changes make collaboration visible without turning the page into a chat app.">
            <div className="mt-6 flex -space-x-3">
              {['AM', 'RK', 'JS', 'NL'].map((x, i) => <span key={x} className="grid size-10 place-items-center rounded-full border-2 border-card text-xs font-semibold text-white shadow" style={{ backgroundColor: ['#2563eb', '#16a34a', '#9333ea', '#ea580c'][i] }}>{x}</span>)}
            </div>
          </Bento>
          <Bento className="lg:col-span-3" icon={<Command className="size-5" />} title="Find any draft in two keystrokes" text="Command palette, recents, and fast switching keep writers in flow." />
          <Bento className="lg:col-span-2" icon={<GitBranch className="size-5" />} title="Reconnect without overwrites" text="Concurrent edits merge cleanly after flaky Wi‑Fi or an offline commute." />
          <Bento className="lg:col-span-2" icon={<MessageSquareText className="size-5" />} title="Less status chatter" text="The document itself shows owners, open decisions, and launch readiness." />
          <Bento className="lg:col-span-2" icon={<Lock className="size-5" />} title="Share the right version" text="Invite links, read-only access, and owner controls prevent accidental leaks." />
        </div>
      </section>

      <section className="border-y border-border/70 bg-muted/35" id="security">
        <div className="mx-auto grid max-w-7xl gap-10 px-5 py-20 sm:px-8 lg:grid-cols-[0.8fr_1.2fr]">
          <div>
            <p className="text-sm font-medium text-primary">Fluid workflow</p>
            <h2 className="mt-3 font-heading text-4xl font-bold tracking-tight sm:text-5xl">One page adapts to every team ritual.</h2>
            <p className="mt-4 text-muted-foreground">Switch context without switching tools. The product frame changes; the writing surface stays familiar.</p>
          </div>
          <div className="rounded-[2rem] border border-border/80 bg-card/70 p-3 shadow-xl backdrop-blur">
            <div className="flex rounded-2xl bg-muted p-1">
              {tabs.map((tab, index) => (
                <button key={tab.name} onClick={() => setActiveTab(index)} className={`flex-1 rounded-xl px-3 py-2 text-sm font-medium transition-all ${activeTab === index ? 'bg-card text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}`}>{tab.name}</button>
              ))}
            </div>
            <div className="mt-4 rounded-2xl border border-border/80 bg-background/70 p-6 transition-all duration-500">
              <div className="flex items-start gap-4">
                <div className="grid size-12 place-items-center rounded-2xl bg-primary text-primary-foreground"><ActiveIcon className="size-5" /></div>
                <div>
                  <h3 className="font-heading text-2xl font-bold tracking-tight">{tabs[activeTab].name} move faster here.</h3>
                  <p className="mt-2 text-muted-foreground">{tabs[activeTab].text}</p>
                </div>
              </div>
              <div className="mt-6 grid gap-3 sm:grid-cols-3">
                {['Draft', 'Align', 'Share'].map((step, i) => <div key={step} className="rounded-xl border border-border/70 bg-card/75 p-4 text-sm"><div className="mb-3 h-1.5 rounded-full bg-primary/30"><div className="h-full rounded-full bg-primary" style={{ width: `${(i + 1) * 32}%` }} /></div>{step}</div>)}
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="mx-auto max-w-7xl px-5 py-20 sm:px-8">
        <div className="ambient-panel group rounded-[2rem] border border-border/80 p-8 shadow-xl shadow-foreground/8 transition hover:-translate-y-1 sm:p-10 lg:flex lg:items-center lg:justify-between">
          <div className="max-w-2xl">
            <div className="mb-4 flex items-center gap-2 text-sm font-medium text-muted-foreground"><ShieldCheck className="size-4" />Private by default</div>
            <h2 className="font-heading text-4xl font-bold tracking-tight">Start with a blank page. Leave with team alignment.</h2>
            <p className="mt-3 text-muted-foreground">Create a shared Markdown draft in seconds. No stock templates, no bloated workspace, no lost decisions.</p>
          </div>
          <Button size="lg" className="mt-8 lg:mt-0" nativeButton={false} render={<Link to="/register" />}>Start writing<ArrowRight className="size-4 transition group-hover:translate-x-1" /></Button>
        </div>
      </section>
    </main>
  );
}

function MegaMenu() {
  return (
    <div className="group relative">
      <button className="flex items-center gap-1 rounded-lg px-3 py-2 text-sm text-muted-foreground transition hover:bg-muted/70 hover:text-foreground">
        Product <ChevronDown className="size-3 transition group-hover:rotate-180" />
      </button>
      <div className="pointer-events-none absolute left-0 top-full z-50 w-[520px] pt-3 opacity-0 transition duration-200 group-hover:pointer-events-auto group-hover:opacity-100">
        <div className="grid grid-cols-2 gap-2 rounded-2xl border border-border/80 bg-popover/95 p-3 shadow-2xl backdrop-blur-xl">
          {[['Live editing', Users], ['Safe sharing', Lock], ['Fast search', Search], ['Markdown focus', FileText]].map(([label, Icon]) => (
            <a key={String(label)} href="#workflow" className="flex gap-3 rounded-xl p-3 text-sm transition hover:bg-muted">
              <Icon className="mt-0.5 size-4 text-primary" />
              <span><span className="block font-medium text-foreground">{label}</span><span className="text-xs text-muted-foreground">Built into the writing surface.</span></span>
            </a>
          ))}
        </div>
      </div>
    </div>
  );
}

function ProductScreenshot({ activeTab }: { activeTab: number }) {
  return (
    <div className="relative mx-auto max-w-6xl rounded-[2rem] border border-border/80 bg-card/80 p-3 shadow-2xl shadow-primary/15 backdrop-blur">
      <div className="absolute -left-4 top-24 hidden rounded-2xl border border-border/80 bg-card/90 p-3 shadow-xl backdrop-blur md:block">
        <MousePointer2 className="mb-2 size-4 text-primary" />
        <p className="text-xs font-medium">Ava is editing scope</p>
      </div>
      <div className="overflow-hidden rounded-[1.5rem] border border-border/80 bg-card">
        <div className="flex items-center justify-between border-b border-border/80 px-4 py-3">
          <div className="flex items-center gap-2"><span className="size-2.5 rounded-full bg-red-400" /><span className="size-2.5 rounded-full bg-amber-400" /><span className="size-2.5 rounded-full bg-emerald-400" /></div>
          <div className="flex items-center gap-2 rounded-full bg-accent px-2.5 py-1 text-xs font-medium text-accent-foreground"><Zap className="size-3" />synced</div>
        </div>
        <div className="grid min-h-[520px] md:grid-cols-[220px_1fr_280px]">
          <aside className="hidden border-r border-border/80 bg-muted/45 p-4 md:block">
            <p className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Drafthouse</p>
            {['Q3 product roadmap', 'Launch checklist', 'Customer notes'].map((doc, index) => <div key={doc} className={`mt-3 rounded-xl p-3 text-xs transition ${index === activeTab ? 'bg-card shadow-sm' : 'text-muted-foreground'}`}><p className="font-medium text-foreground">{doc}</p><p className="mt-1 text-muted-foreground">{index === 0 ? 'Live now' : 'Yesterday'}</p></div>)}
          </aside>
          <div className="p-5">
            <div className="mb-5 flex flex-wrap items-center gap-2 border-b border-border pb-3 text-xs text-muted-foreground"><span className="rounded-md bg-muted px-2 py-1">H1</span><span className="rounded-md bg-muted px-2 py-1">B</span><span className="rounded-md bg-muted px-2 py-1">I</span><span className="ml-auto rounded-md bg-primary px-2 py-1 text-primary-foreground shadow-sm">Preview</span></div>
            <article className="prose prose-sm max-w-none dark:prose-invert prose-headings:font-heading">
              <h2>{tabs[activeTab].name === 'Specs' ? 'Q3 product roadmap' : tabs[activeTab].name === 'Launches' ? 'Beta launch notes' : 'Customer research synthesis'}</h2>
              <p>Align on scope, edge cases, owners, and open decisions before the next milestone.</p>
              <ul><li>Resolve invite permissions</li><li>Stress test reconnect replay</li><li>Publish onboarding doc</li></ul>
            </article>
            <div className="mt-8 rounded-2xl border border-dashed border-primary/35 bg-secondary/60 p-4"><div className="flex items-center gap-2 text-sm font-medium"><Sparkles className="size-4 text-amber-500" />Changes save continuously</div><p className="mt-2 text-xs leading-5 text-muted-foreground">Every edit is saved and safely merged after reconnects.</p></div>
          </div>
          <aside className="hidden border-l border-border/80 bg-muted/35 p-4 lg:block">
            <p className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Presence</p>
            {['Ava writing', 'Ravi reviewing', 'Jess viewing'].map((item, i) => <div key={item} className="mt-4 flex items-center gap-3 rounded-xl bg-card/70 p-3 text-sm"><span className="size-2 rounded-full" style={{ backgroundColor: ['#2563eb', '#16a34a', '#9333ea'][i] }} />{item}</div>)}
            <div className="mt-6 rounded-2xl border border-border/80 bg-card/70 p-4 text-sm"><Layers3 className="mb-3 size-4 text-primary" />3 open decisions framed for sign-off.</div>
          </aside>
        </div>
      </div>
    </div>
  );
}

function Bento({ className = '', icon, title, text, children }: { className?: string; icon: React.ReactNode; title: string; text: string; children?: React.ReactNode }) {
  return (
    <div className={`ambient-panel group rounded-[2rem] border border-border/80 p-6 shadow-sm transition duration-300 hover:-translate-y-1 hover:border-primary/35 hover:shadow-xl ${className}`}>
      <div className="mb-5 grid size-11 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-sm shadow-primary/25 transition group-hover:scale-105">{icon}</div>
      <h3 className="font-heading text-2xl font-bold tracking-tight">{title}</h3>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{text}</p>
      {children}
    </div>
  );
}

function GridLines() {
  return <div className="pointer-events-none absolute inset-y-0 left-1/2 -z-10 w-screen max-w-7xl -translate-x-1/2 border-x border-border/50 [background-image:linear-gradient(to_right,color-mix(in_oklch,var(--border)_45%,transparent)_1px,transparent_1px)] [background-size:25%_100%]" />;
}
