/**
 * /design — a living style guide. Renders every design token and UI primitive
 * so changes can be eyeballed in one place. Dev-only (reached via ⌘K).
 */
import { useState, type ReactNode } from "react";

import { ThemeToggle } from "@/components/ThemeToggle";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Dialog,
  Input,
  PagePreview,
  Select,
  Separator,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Textarea,
  Tooltip,
} from "@/components/ui";

const SEMANTIC = [
  "--color-bg",
  "--color-bg-elevated",
  "--color-bg-surface",
  "--color-fg",
  "--color-fg-muted",
  "--color-border",
  "--color-accent",
  "--color-brand",
];
const STATUS = [
  "--color-success",
  "--color-warning",
  "--color-danger",
  "--color-info",
];
const ACCENTS = [
  "--color-copper-400",
  "--color-emerald-400",
  "--color-indigo-400",
];
const UI_SCALE = ["xs", "sm", "md", "lg", "xl", "2xl", "3xl"] as const;
const DOC_SCALE = ["xs", "sm", "md", "lg", "xl", "2xl"] as const;

export function DesignPage() {
  const [tab, setTab] = useState("forward");
  const [dialogOpen, setDialogOpen] = useState(false);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-4xl px-8 py-10">
        <header className="mb-10 flex items-start justify-between">
          <div>
            <div className="mb-1 text-xs font-medium tracking-widest text-[var(--color-accent)] uppercase">
              Designsystem
            </div>
            <h1 className="text-[var(--text-ui-3xl)] font-bold">
              SundayPaper UI
            </h1>
            <p className="mt-1 text-sm text-[var(--color-fg-muted)]">
              Levende stilguide — tokens og primitiver.
            </p>
          </div>
          <ThemeToggle />
        </header>

        <Section title="Farger — semantiske">
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {SEMANTIC.map((v) => (
              <Swatch key={v} token={v} />
            ))}
          </div>
        </Section>

        <Section title="Farger — status">
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {STATUS.map((v) => (
              <Swatch key={v} token={v} />
            ))}
          </div>
        </Section>

        <Section title="Aksent-kandidater (copper er standard)">
          <div className="grid grid-cols-3 gap-3">
            {ACCENTS.map((v) => (
              <Swatch key={v} token={v} />
            ))}
          </div>
        </Section>

        <Section title="Typografi — UI-skala (Inter)">
          <div className="space-y-1">
            {UI_SCALE.map((s) => (
              <p key={s} style={{ fontSize: `var(--text-ui-${s})` }}>
                <span className="mr-3 font-mono text-xs text-[var(--color-fg-muted)]">
                  ui-{s}
                </span>
                Søndagens program
              </p>
            ))}
          </div>
        </Section>

        <Section title="Typografi — dokument-skala (serif)">
          <div
            className="space-y-1"
            style={{ fontFamily: "var(--font-serif)" }}
          >
            {DOC_SCALE.map((s) => (
              <p key={s} style={{ fontSize: `var(--text-doc-${s})` }}>
                <span className="mr-3 font-mono text-xs text-[var(--color-fg-muted)]">
                  doc-{s}
                </span>
                Nådens evangelium
              </p>
            ))}
          </div>
        </Section>

        <Section title="Knapper">
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="primary">Primary</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="outline">Outline</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="danger">Danger</Button>
            <Button disabled>Disabled</Button>
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button size="sm">Small</Button>
            <Button size="md">Medium</Button>
            <Button size="lg">Large</Button>
          </div>
        </Section>

        <Section title="Skjemakontroller">
          <div className="grid max-w-md gap-3">
            <Input placeholder="Dokumenttittel…" />
            <Textarea placeholder="Kunngjøring…" />
            <Select defaultValue="a4">
              <option value="a4">A4</option>
              <option value="letter">US Letter</option>
            </Select>
          </div>
        </Section>

        <Section title="Merker">
          <div className="flex flex-wrap gap-2">
            <Badge variant="neutral">Utkast</Badge>
            <Badge variant="accent">AI</Badge>
            <Badge variant="success">Klar</Badge>
            <Badge variant="warning">Mangler data</Badge>
            <Badge variant="danger">Feil</Badge>
          </div>
        </Section>

        <Section title="Kort">
          <Card className="max-w-sm">
            <CardHeader>
              <CardTitle>Søndagsprogram</CardTitle>
              <CardDescription>
                4 sanger · 1 tekstlesning · 2 kunngjøringer
              </CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-[var(--color-fg-muted)]">
                Generert fra serviceplanen. Klar for eksport.
              </p>
            </CardContent>
            <CardFooter>
              <Button size="sm">Åpne</Button>
              <Button size="sm" variant="outline">
                Eksporter
              </Button>
            </CardFooter>
          </Card>
        </Section>

        <Section title="Faner">
          <Tabs value={tab} onValueChange={setTab} className="space-y-3">
            <TabsList>
              <TabsTrigger value="forward">Generer</TabsTrigger>
              <TabsTrigger value="backward">Innta</TabsTrigger>
            </TabsList>
            <TabsContent value="forward">
              <p className="text-sm text-[var(--color-fg-muted)]">
                Intent / data → blokktre → PDF.
              </p>
            </TabsContent>
            <TabsContent value="backward">
              <p className="text-sm text-[var(--color-fg-muted)]">
                PDF → klipp / OCR / flett → ressursbibliotek.
              </p>
            </TabsContent>
          </Tabs>
        </Section>

        <Section title="Dialog + Tooltip">
          <div className="flex items-center gap-3">
            <Button onClick={() => setDialogOpen(true)}>Åpne dialog</Button>
            <Tooltip label="Forklaring som dukker opp">
              <Button variant="outline">Hold over meg</Button>
            </Tooltip>
          </div>
          <Dialog
            open={dialogOpen}
            onClose={() => setDialogOpen(false)}
            title="Slette dokument?"
            description="Dette kan ikke angres."
            footer={
              <>
                <Button variant="ghost" onClick={() => setDialogOpen(false)}>
                  Avbryt
                </Button>
                <Button variant="danger" onClick={() => setDialogOpen(false)}>
                  Slett
                </Button>
              </>
            }
          />
        </Section>

        <Section title="Sideforhåndsvisning (dokument-primitiv)">
          <div className="flex flex-wrap items-end gap-4">
            <PagePreview paper="a4" pageNumber={1} label="Forside" selected />
            <PagePreview paper="a4" pageNumber={2} label="Sang 1" />
            <PagePreview paper="letter" pageNumber={3} label="Letter" />
          </div>
        </Section>

        <Separator className="my-8" />
        <p className="pb-8 text-center text-xs text-[var(--color-fg-muted)]">
          Tokens i <code className="font-mono">src/styles/tokens.css</code> ·
          primitiver i <code className="font-mono">src/components/ui</code>
        </p>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-10">
      <h2 className="mb-3 text-[var(--text-ui-sm)] font-semibold tracking-wide text-[var(--color-fg-muted)] uppercase">
        {title}
      </h2>
      {children}
    </section>
  );
}

function Swatch({ token }: { token: string }) {
  return (
    <div className="overflow-hidden rounded-lg border border-[var(--color-border)]">
      <div className="h-12 w-full" style={{ background: `var(${token})` }} />
      <div className="bg-[var(--color-bg-elevated)] px-2 py-1.5">
        <code className="font-mono text-[10px] text-[var(--color-fg-muted)]">
          {token.replace("--color-", "")}
        </code>
      </div>
    </div>
  );
}
