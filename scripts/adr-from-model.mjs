// Mirror the Atlas model's decisions into docs/adr/ as one file per ADR.
//
// The model is the source of truth (Atlas system `reviewglass`, `decisions[]`); these
// files are a rendering of it, in the family's ADR layout, so the decisions can be read
// in the repository without Atlas. Regenerate after every model change:
//
//   node scripts/adr-from-model.mjs [path-to-reviewglass.model.json]
//
// The default path is the sibling Atlas checkout. A hand edit to a generated file is
// lost on the next run — edit the model, then regenerate.

import { readFileSync, writeFileSync, mkdirSync, readdirSync, unlinkSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const modelPath =
  process.argv[2] ??
  join(here, "..", "..", "quaesitor atlas", "model", "systems", "reviewglass.model.json");
const outDir = join(here, "..", "docs", "adr");

const model = JSON.parse(readFileSync(modelPath, "utf8"));
const byId = new Map(model.modules.map((m) => [m.id, m.name]));
mkdirSync(outDir, { recursive: true });

// Start clean so a renumbered or removed decision cannot leave a stale file behind.
for (const f of readdirSync(outDir)) if (/^\d{3,4}-.*\.md$/.test(f)) unlinkSync(join(outDir, f));

const slug = (s) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 48);

const index = [];
for (const d of model.decisions) {
  const n = d.id.replace(/^adr\.rg\./, "").padStart(4, "0");
  const file = `${n}-${slug(d.title)}.md`;
  const links = (d.links ?? [])
    .map((l) => (byId.has(l) ? `\`${l}\` (${byId.get(l)})` : `\`${l}\``))
    .join(", ");
  const status = (d.status ?? "proposed").replace(/^\w/, (c) => c.toUpperCase());
  const body = `# ADR-${n}: ${d.title}

**Status:** ${status}
**Atlas id:** \`${d.id}\`
**Links:** ${links || "—"}

> Rendered from the Atlas model (system \`reviewglass\`). The model is the source of
> truth; edit it there and run \`node scripts/adr-from-model.mjs\`.

---

## Context

${d.context}

## Decision

${d.decision}

## Consequences

${d.consequences ?? "—"}
`;
  writeFileSync(join(outDir, file), body);
  index.push(`- [ADR-${n}](${file}) — ${d.title} *(${d.status})*`);
}

writeFileSync(
  join(outDir, "README.md"),
  `# Architecture decision records

One file per decision, rendered from the Atlas model's \`decisions[]\` for system
\`reviewglass\`. The model is the source of truth; these files exist so the decisions
are readable in the repository without Atlas. Regenerate with:

\`\`\`bash
node scripts/adr-from-model.mjs
\`\`\`

A new decision is added to the model (a proposal or a PR against the Atlas repo), not
here. A hand edit here is lost on the next run.

${index.join("\n")}
`,
);
console.log(`${model.decisions.length} ADRs -> ${outDir}`);
