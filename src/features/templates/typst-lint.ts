/**
 * Pure, dependency-free helpers for the Typst template builder.
 *
 * The Template editor is a thin shell around three pure functions kept here so
 * they can be unit-tested without React or a backend:
 *
 *   - `extractVariables`  — find every `{{ var }}` placeholder in the source
 *   - `injectSampleData`  — substitute sample values for a clean preview source
 *   - `validateTypst`     — catch the cheap-to-detect mistakes a volunteer
 *                           makes (unbalanced braces, dangling `#`, unknown
 *                           placeholder), BEFORE we pay for a Typst compile
 *
 * `{{VAR}}` is the same placeholder syntax `doc_template_render` uses on the
 * Rust side, so a template authored here renders the same way once bound to a
 * document. Sample data lets the preview show a realistic layout while the
 * author is still deciding which variables to expose.
 */

/** A placeholder discovered in the template source. */
export interface TemplateVariable {
  /** The bare name, e.g. `church_name` for `{{ church_name }}`. */
  name: string;
  /** 1-based count of occurrences across the source. */
  count: number;
}

/** A single problem found by {@link validateTypst}. */
export interface LintIssue {
  severity: "error" | "warning";
  message: string;
}

// Placeholders look like `{{ name }}` — letters, digits and underscores, with
// optional surrounding whitespace. The capture group is the bare name.
const PLACEHOLDER_RE = /\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g;

/**
 * Collect every `{{ var }}` placeholder, de-duplicated and ordered by first
 * appearance, with an occurrence count. Drives the "variable hints" rail.
 */
export function extractVariables(source: string): TemplateVariable[] {
  const order: string[] = [];
  const counts = new Map<string, number>();
  for (const m of source.matchAll(PLACEHOLDER_RE)) {
    const name = m[1];
    if (!counts.has(name)) order.push(name);
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return order.map((name) => ({ name, count: counts.get(name) ?? 0 }));
}

/**
 * Replace each `{{ var }}` with a sample value. Missing keys fall back to a
 * visible placeholder so the author can spot an unfilled slot in the preview.
 */
export function injectSampleData(
  source: string,
  samples: Record<string, string>,
): string {
  return source.replace(PLACEHOLDER_RE, (_full, name: string) => {
    const value = samples[name];
    return value !== undefined ? value : `[${name}]`;
  });
}

/**
 * Cheap structural lint over RAW template source (placeholders intact). Catches
 * the common Typst slips before a compile round-trip:
 *
 *   - unbalanced `{ }` / `[ ]` / `( )` (excludes `{{…}}` placeholders)
 *   - a `#` with nothing after it (dangling function/value marker)
 *   - a malformed placeholder such as `{{ }}` or `{{ 9x }}`
 *
 * Returns `[]` for source that passes the cheap checks — it does NOT guarantee
 * the Typst compiler will accept it, only that the obvious mistakes are gone.
 */
export function validateTypst(source: string): LintIssue[] {
  const issues: LintIssue[] = [];

  // Strip valid placeholders so their braces don't skew the balance check.
  const stripped = source.replace(PLACEHOLDER_RE, "");

  // A leftover `{{` or `}}` after stripping means a malformed placeholder.
  if (/\{\{|\}\}/.test(stripped)) {
    issues.push({
      severity: "error",
      message:
        "Ugyldig variabel — bruk {{ navn }} med bokstaver, tall eller _.",
    });
  }

  // Balance the three bracket pairs on the stripped source.
  const pairs: Array<[string, string, string]> = [
    ["{", "}", "krøllparentes { }"],
    ["[", "]", "hakeparentes [ ]"],
    ["(", ")", "parentes ( )"],
  ];
  for (const [open, close, label] of pairs) {
    let depth = 0;
    let broke = false;
    for (const ch of stripped) {
      if (ch === open) depth++;
      else if (ch === close) {
        depth--;
        if (depth < 0) {
          broke = true;
          break;
        }
      }
    }
    if (broke || depth !== 0) {
      issues.push({
        severity: "error",
        message: `Ubalansert ${label}.`,
      });
    }
  }

  // A `#` immediately followed by end-of-line / end-of-source / whitespace is a
  // dangling marker — Typst expects a function name or value after it.
  if (/#(\s|$)/.test(stripped)) {
    issues.push({
      severity: "warning",
      message: "«#» uten uttrykk etter seg — la til en funksjon eller verdi?",
    });
  }

  return issues;
}
