#!/usr/bin/env node
/* Run the fast search over the shipped LilScript libraries.
 *
 * For each artifact: build the factorial grid, take the best point, verify the
 * rewrite is legal (binding graph and resolution sequence), and score baseline
 * and winner with `lilscript-codec` — the gate, not the diagnostic scorer.
 *
 * The grid is 32–64 evaluations and takes seconds. The compiler's own
 * candidate search did not finish one 100 KB artifact in 4.5 hours, so this is
 * also a statement about where the time is going.
 *
 * Usage: node libraries.mjs [--only <substring>] [--quick]
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { analyze, verify } from "./scope.mjs";
import { verifyReorder } from "./pool.mjs";
import { factorsFor, build, gridSize, pointAt, levelsOf } from "./factorial.mjs";
import { brotli } from "./census.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const CODEC = "/Users/yeargun/lilscript/target/release/lilscript-codec";
const WORK = "/private/tmp/claude-501/-Users-yeargun-lilscript/68d4d12f-89ad-4d08-8494-1336a12a22e8/scratchpad/lib";

export const LIBRARIES = [
  ["markedlil", "marked.raw.js", "/Users/yeargun/markedlil/dist/marked.raw.js"],
  ["motionlil", "mini.js", "/Users/yeargun/motionlil/dist/mini.js"],
  ["motionlil", "scroll.js", "/Users/yeargun/motionlil/dist/scroll.js"],
  ["motionlil", "animate.js", "/Users/yeargun/motionlil/dist/animate.js"],
  ["motionlil", "index.bundle.js", "/Users/yeargun/motionlil/dist/index.bundle.js"],
  ["motionlil", "full.js", "/Users/yeargun/motionlil/dist/full.js"],
  ["posthoglil", "surveys.raw.js", "/Users/yeargun/posthoglil/dist/surveys.raw.js"],
  ["posthoglil", "otlp.raw.js", "/Users/yeargun/posthoglil/dist/otlp.raw.js"],
  ["posthoglil", "replay-core.raw.js", "/Users/yeargun/posthoglil/dist/replay-core.raw.js"],
  ["posthoglil", "autocapture.raw.js", "/Users/yeargun/posthoglil/dist/autocapture.raw.js"],
  ["posthoglil", "posthog.raw.js", "/Users/yeargun/posthoglil/dist/posthog.raw.js"],
  ["posthoglil", "error-tracking.raw.js", "/Users/yeargun/posthoglil/dist/error-tracking.raw.js"],
  ["jquerylil", "jquery.esm.js", "/Users/yeargun/jquerylil/dist/jquery.esm.js"],
  ["solidlil", "reactive.generated.js", "/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/reactive.generated.js"],
];

/* Which factors carry a proof, and which need a battery.
     N, O — renaming: positions are preserved, so the resolution sequence is a
            proof.
     D    — declaration merging: statement-level, checked by the same sequence.
     P    — pool order: permutation, proved by canonicalisation.
     W, C — control-flow spelling and outlining: no structural proof exists
            (outlining adds bindings by construction), so they are excluded
            unless a behavioural battery runs. */
const PROVABLE = new Set(["N", "O", "M", "D", "P"]);

/* Apply the winning point one factor at a time, proving each step against its
   own input with the check that factor admits. A composite rewrite has no
   single proof; a chain of proved steps does. */
export function buildVerified(source, factors, index) {
  const point = pointAt(factors, index);
  let text = source;
  const checks = [];
  factors.forEach((f, i) => {
    const level = levelsOf(f)[point[i]];
    if (level.label === "off" || level.label === "as shipped") return;
    const next = level.apply(text);
    if (next === text) return;
    let check;
    if (f.key === "P") check = verifyReorder(text, next);
    else check = verify(analyze(text, { renameModuleTopLevel: true }), next, new Map());
    checks.push({ factor: f.key, level: level.label, ok: check.ok, why: check.why || null });
    text = next;
  });
  return { text, checks, ok: checks.every((c) => c.ok) };
}

export function searchArtifact(source, { quick = false, provableOnly = true } = {}) {
  const factors = factorsFor(source).filter((f) => !provableOnly || PROVABLE.has(f.key));
  const total = gridSize(factors);
  const limit = quick ? Math.min(total, 16) : total;
  let best = null;
  let evaluated = 0;
  for (let i = 0; i < limit; i++) {
    let text;
    try { text = build(source, factors, i); } catch { continue; }
    evaluated++;
    const size = brotli(text).length;
    if (!best || size < best.size) best = { i, size, text };
  }
  const point = pointAt(factors, best.i)
    .map((l, k) => `${factors[k].key}=${levelsOf(factors[k])[l].label}`)
    .filter((s) => !s.endsWith("=off"))
    .join(" ") || "baseline";
  return { best, point, evaluated, factors };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { mkdirSync } = await import("node:fs");
  mkdirSync(WORK, { recursive: true });
  const flag = process.argv.indexOf("--only");
  const filter = flag >= 0 ? process.argv[flag + 1] : null;
  const quick = process.argv.includes("--quick");

  const rows = [];
  for (const [project, name, path] of LIBRARIES) {
    if (filter && !`${project}/${name}`.includes(filter)) continue;
    if (!existsSync(path)) { console.log(`${project}/${name}: MISSING`); continue; }
    const source = readFileSync(path, "utf8");
    let result, legal = "n/a";
    const started = Date.now();
    try {
      result = searchArtifact(source, { quick });
      const staged = buildVerified(source, result.factors, result.best.i);
      if (staged.text !== result.best.text) {
        legal = "stagewise rebuild differs from the searched text";
      } else {
        legal = staged.ok ? "ok" : staged.checks.filter((c) => !c.ok).map((c) => `${c.factor}: ${c.why}`).join("; ");
      }
      result.checks = staged.checks;
    } catch (e) {
      console.log(`${(project + "/" + name).padEnd(34)} FAILED ${e.message}`);
      continue;
    }
    const seconds = (Date.now() - started) / 1000;
    const outPath = join(WORK, `${project}-${name}`);
    writeFileSync(outPath, result.best.text);
    const codec = JSON.parse(execFileSync(CODEC, ["--json", path, outPath], { encoding: "utf8" }));
    const [a, b] = codec.artifacts;
    const row = {
      project, name, evaluated: result.evaluated, seconds,
      point: result.point, legal,
      base: { raw: a.raw, gzip9: a.gzip9, br11: a.brotli11 },
      best: { raw: b.raw, gzip9: b.gzip9, br11: b.brotli11 },
      delta: { raw: b.raw - a.raw, gzip9: b.gzip9 - a.gzip9, br11: b.brotli11 - a.brotli11 },
      percent: ((b.brotli11 - a.brotli11) / a.brotli11) * 100,
    };
    rows.push(row);
    console.log(`${(project + "/" + name).padEnd(34)} ${String(a.brotli11).padStart(7)} → ${String(b.brotli11).padStart(7)} br11` +
      `  ${(row.delta.br11 >= 0 ? "+" : "") + row.delta.br11}`.padStart(8) +
      `  ${row.percent.toFixed(2)}%`.padStart(8) +
      `   raw ${(row.delta.raw >= 0 ? "+" : "") + row.delta.raw}`.padStart(13) +
      `   ${result.evaluated} evals in ${seconds.toFixed(1)}s   ${legal === "ok" ? "legal" : "CHECK: " + legal}`);
    if (row.delta.br11 < 0) console.log(`${" ".repeat(34)} ${result.point}`);
  }

  const totals = rows.reduce((a, r) => ({
    base: a.base + r.base.br11, best: a.best + r.best.br11, seconds: a.seconds + r.seconds,
  }), { base: 0, best: 0, seconds: 0 });
  console.log(`\n${rows.length} artifacts: ${totals.base} → ${totals.best} Brotli bytes` +
    `  (${totals.best - totals.base}, ${(((totals.best - totals.base) / totals.base) * 100).toFixed(2)}%)` +
    `  in ${totals.seconds.toFixed(0)}s of search`);
  writeFileSync(join(here, "libraries.json"), JSON.stringify(rows, null, 1));
  console.log("wrote libraries.json");
}
