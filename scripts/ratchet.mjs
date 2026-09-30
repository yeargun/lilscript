#!/usr/bin/env node
// The generic corpus ratchet (plan task M2.13; architecture §18.3 NO2, laws L21).
//
//   node scripts/ratchet.mjs --compiler <lilscript> [--codec <lilscript-codec>]
//        [--sets cases,apps,algorithms] [--filter <id-substring|glob>,...]
//        [--jobs N] [--work DIR] [--json out.json] [--markdown out.md]
//        [--format bare|iife] [--bars FILE] [--baseline FILE] [--ledger FILE] [--update-baseline [--accept-growth]] [--verbose]
//        [--config-dir DIR] [--retain-explanations]
//   node scripts/ratchet.mjs --refresh-bars --compiler <lilscript> --reference <old-route lilscript>
//
// Three generic corpora, none of them a port: comparison/cases (54 canonical
// folders and the 570 catalog variants), comparison/apps (7 programs written
// for Closure ADVANCED) and comparison/algorithms (11 host-fed programs). Each
// item compiles once per objective lane (raw, gzip, Brotli) with the corpus's
// checked-in configuration, runs against its oracle (the original JavaScript's
// stdout; the algorithms also check every host access, in order), and is
// measured with the canonical codec. Every passing artifact is then compared,
// in its own metric, with three bars fixed in tests/ratchet/bars.json:
//   competitor  the smallest valid Terser, Oxc or esbuild artifact (and for the
//               algorithms the bundler lanes of their harness)
//   closure     Closure ADVANCED (apps and algorithms only)
//   old         the frozen old route: reference-2026-09-23 --backend legacy
// An item loses to a bar when its artifact is larger than the bar.
//
// The ratchet (plan rule 3): tests/ratchet/baseline.json holds every item's
// sizes from the last accepted run. A run fails (exit 1) when
//   - any loss count (per corpus, metric and bar) grows,
//   - any item's loss to any bar grows (a new loss is growth from zero),
//   - an item that compiled and passed its oracle now fails,
//   - a loss or failure is not covered by a ledger entry with an owner
//     (tests/ratchet/ledger.json), or the bars changed under the baseline.
// Improvements are reported; --update-baseline writes them into the baseline,
// and refuses while anything above fails. With --accept-growth it also accepts
// items whose loss grew, and loss counts that grew, when no corpus total
// grew in any metric (the owner's ruling of 2026-09-29: a batch is judged by
// its totals, and a few bytes lost locally for an overall win are accepted);
// it lists what it accepts. Bars are refreshed only with
// --refresh-bars, a scheduled re-baseline event (BC3) that needs the pinned
// competitors (benchmarks/popular), Closure and the reference binary; the gate
// itself needs only Node, the compiler and the codec.
//
// See docs/testing.md.
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { globMatcher, headLines, pinBinary, pool, repository, run, sha256, sha256File } from "./lib/verify-util.mjs";

export const SETS = ["cases", "apps", "algorithms"];
export const LANES = [
  { id: "raw", metric: "raw" },
  { id: "gzip", metric: "gzip9" },
  { id: "brotli", metric: "brotli11" },
];
export const METRICS = LANES.map((lane) => lane.metric);
export const BARS = ["competitor", "closure", "old"];
export const FAILURES = ["refused", "compiler-crash", "crashed", "wrong-output"];
const CONFIGS = {
  cases: "comparison/cases/configs",
  apps: "comparison/cases/configs",
  algorithms: "comparison/algorithms/configs",
};
const OLD_ROUTE_CONFIGS = "tests/ratchet/old-route";
const DEFAULTS = {
  bars: "tests/ratchet/bars.json",
  baseline: "tests/ratchet/baseline.json",
  ledger: "tests/ratchet/ledger.json",
};
const casesRoot = join(repository, "comparison/cases");
const appsRoot = join(repository, "comparison/apps");
const algorithmsRoot = join(repository, "comparison/algorithms");
// Compiler search is controlled by the retained TOML, with no ambient audit
// adapter or optional timing instrumentation changing its work.
const compilerEnvironment = { ...process.env };
delete compilerEnvironment.LILSCRIPT_WALK_AUDIT;
delete compilerEnvironment.LILSCRIPT_TIMING;

// ---------------------------------------------------------------- corpus

function walkCanonical(root) {
  const found = [];
  const walk = (directory) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (name === "case.toml") found.push(directory);
    }
  };
  if (existsSync(root)) walk(root);
  return found;
}

// The catalog oracle digest exactly as comparison/cases/run.mjs computes it, so
// the ratchet refuses a corpus whose reference programs or stdout drifted.
export function catalogOracleDigest(entries, stdoutByName) {
  const records = entries.map((entry) => ({
    name: entry.name,
    behavior: entry.behavior,
    terserProperties: entry.terserProperties,
    terserPropertyReason: entry.terserPropertyReason,
    expect: entry.expect,
    javascript: entry.js,
    stdout: stdoutByName.get(entry.name),
  }));
  return sha256(JSON.stringify(records));
}

// A catalog variant's id carries its behavior family (the first segment of its
// behavior id), so ledger entries can name a family: cases/catalog/loop/*.
export function catalogId(behavior, name) {
  return `cases/catalog/${behavior.split("/")[0]}/${name}`;
}

export async function loadCorpus(sets) {
  const items = [];
  const catalogEntries = [];
  if (sets.includes("cases")) {
    for (const folder of walkCanonical(join(casesRoot, "canonical"))) {
      const name = relative(join(casesRoot, "canonical"), folder).replaceAll("\\", "/");
      items.push({
        id: `cases/canonical/${name}`, set: "cases", kind: "canonical",
        entry: join(folder, "main.lil"), js: readFileSync(join(folder, "main.js"), "utf8"),
      });
    }
    const { catalog } = await import(pathToFileURL(join(casesRoot, "catalog.mjs")));
    for (const entry of catalog()) {
      catalogEntries.push(entry);
      items.push({ id: catalogId(entry.behavior, entry.name), set: "cases", kind: "catalog", name: entry.name, lil: entry.lil, js: entry.js });
    }
  }
  if (sets.includes("apps")) {
    for (const name of readdirSync(appsRoot).sort()) {
      const folder = join(appsRoot, name);
      if (!existsSync(join(folder, "lilscript/main.lil"))) continue;
      items.push({
        id: `apps/${name}`, set: "apps", kind: "app", folder,
        entry: join(folder, "lilscript/main.lil"), expected: readFileSync(join(folder, "tests/stdout.txt"), "utf8"),
      });
    }
  }
  if (sets.includes("algorithms")) {
    for (const name of readdirSync(join(algorithmsRoot, "cases")).sort()) {
      const folder = join(algorithmsRoot, "cases", name);
      if (!existsSync(join(folder, "case.json"))) continue;
      const metadata = JSON.parse(readFileSync(join(folder, "case.json"), "utf8"));
      items.push({
        id: `algorithms/${name}`, set: "algorithms", kind: "algorithm", folder,
        entry: join(folder, "main.lil"), vectors: metadata.vectors,
      });
    }
  }
  return { items, catalogEntries };
}

export function selectItems(items, filter) {
  if (!filter) return items;
  const matchers = filter.split(",").map((item) => item.trim()).filter(Boolean)
    .map((item) => (item.includes("*") ? globMatcher(item) : (id) => id.includes(item)));
  return items.filter((item) => matchers.some((match) => match(item.id)));
}

// ---------------------------------------------------------------- execution

function keyLine(stderr) {
  const lines = stderr.split("\n").map((line) => line.trim()).filter(Boolean);
  return (lines.find((line) => /\berror\b|panicked|Error/.test(line) && !line.startsWith("warning")) ?? lines.find((line) => !line.startsWith("warning")) ?? "").slice(0, 300);
}

async function executeScript(code, { cwd, env, preload, timeoutMs }) {
  const args = [...(preload ? ["--require", preload] : []), "--input-type=commonjs"];
  return run(process.execPath, args, { cwd, env: env ?? process.env, timeoutMs, input: code });
}

// Expected stdout of a reference JavaScript program, cached by source digest.
async function referenceStdout(js, cache) {
  const key = sha256(js);
  if (cache.has(key)) return cache.get(key);
  const result = await executeScript(js, { cwd: casesRoot, timeoutMs: 10_000 });
  if (result.status !== 0) throw new Error(`reference JavaScript failed: ${headLines(result.stderr, 3)}`);
  cache.set(key, result.stdout);
  return result.stdout;
}

const TRACE = "LILSCRIPT_ALGORITHM_TRACE=";

async function runAlgorithm(code, vectors, traces, timeoutMs) {
  for (const vector of vectors) {
    const env = {
      ...process.env,
      LILSCRIPT_ALGORITHM_TRACE: "1",
      LILSCRIPT_ALGORITHM_VECTOR: JSON.stringify({ ints: vector.ints, strings: vector.strings }),
    };
    const result = await executeScript(code, { cwd: algorithmsRoot, env, preload: join(algorithmsRoot, "host.cjs"), timeoutMs });
    if (result.status !== 0 || result.timedOut) {
      return { state: "crashed", detail: `vector ${vector.name}: ${keyLine(result.stderr) || `exit ${result.status}`}` };
    }
    const lines = result.stderr.split("\n").filter(Boolean);
    const traceLines = lines.filter((line) => line.startsWith(TRACE));
    if (traceLines.length !== 1 || lines.length !== 1) {
      return { state: "wrong-output", detail: `vector ${vector.name}: invalid host trace` };
    }
    if (result.stdout !== vector.expected) {
      return { state: "wrong-output", detail: `vector ${vector.name}: stdout ${JSON.stringify(result.stdout.slice(0, 80))}` };
    }
    const expected = traces?.[vector.name];
    if (expected && traceLines[0].slice(TRACE.length) !== JSON.stringify(expected)) {
      return { state: "wrong-output", detail: `vector ${vector.name}: host accesses differ from the reference program` };
    }
  }
  return { state: "pass" };
}

// Compile one item for one objective lane, run its oracle. Never throws.
async function compileAndCheck({ binary, extraArgs, configDir, item, lane, work, oracle, timeoutMs, retainExplanations }) {
  const directory = join(work, "out", binary.label, item.id);
  mkdirSync(directory, { recursive: true });
  const output = join(directory, `${lane.id}.js`);
  rmSync(output, { force: true });
  const config = resolve(repository, configDir, `${lane.id}.toml`);
  const compile = await run(binary.path, [item.entry, ...extraArgs, "--config", config, "--target", "js", "--mode", "production", "-o", output], {
    cwd: dirname(item.entry), timeoutMs, env: compilerEnvironment,
  });
  if (retainExplanations) writeFileSync(join(directory, `${lane.id}.explain.txt`), compile.stderr);
  if (compile.timedOut || compile.signal || compile.status === 101) {
    return { state: "compiler-crash", detail: compile.timedOut ? "timed out" : keyLine(compile.stderr) };
  }
  if (compile.status !== 0 || !existsSync(output)) return { state: "refused", detail: keyLine(compile.stderr) };
  const code = readFileSync(output, "utf8");
  const record = { path: relative(work, output), sha256: sha256(code), bytes: Buffer.byteLength(code) };
  let verdict;
  if (item.set === "algorithms") {
    verdict = await runAlgorithm(code, item.vectors, oracle.traces?.[item.id], 10_000);
  } else {
    const result = await executeScript(code, { cwd: item.set === "apps" ? item.folder : casesRoot, timeoutMs: 10_000 });
    if (result.status !== 0 || result.timedOut) verdict = { state: "crashed", detail: keyLine(result.stderr) || `exit ${result.status}` };
    else if (result.stdout !== item.expected) verdict = { state: "wrong-output", detail: `stdout ${JSON.stringify(result.stdout.slice(0, 80))}` };
    else verdict = { state: "pass" };
  }
  return { ...verdict, artifact: record };
}

async function measure(codec, files) {
  const sizes = new Map();
  const unique = [...new Map(files.map((file) => [file.sha256, file.path])).entries()];
  for (let start = 0; start < unique.length; start += 200) {
    const batch = unique.slice(start, start + 200);
    const result = await run(codec.path, ["--json", ...batch.map(([, path]) => path)], { timeoutMs: 600_000 });
    if (result.status !== 0) throw new Error(`codec failed: ${headLines(result.stderr, 3)}`);
    JSON.parse(result.stdout).artifacts.forEach((row, index) => {
      sizes.set(batch[index][0], { raw: row.raw, gzip9: row.gzip9, brotli11: row.brotli11 });
    });
  }
  return sizes;
}

// Every selected item in every lane: { id: { lane metric: {state, size?, detail?} } }.
async function compileCorpus({ binary, extraArgs, configFor, items, work, jobs, codec, oracle, retainExplanations = false }) {
  const tasks = items.flatMap((item) => LANES.map((lane) => ({ item, lane })));
  let done = 0;
  const results = await pool(tasks, jobs, async ({ item, lane }) => {
    const outcome = await compileAndCheck({ binary, extraArgs, configDir: configFor(item), item, lane, work, oracle, timeoutMs: 180_000, retainExplanations });
    done += 1;
    if (done % 300 === 0 || done === tasks.length) process.stderr.write(`  ${binary.label}: ${done}/${tasks.length} compiled and run\n`);
    return { item, lane, outcome };
  });
  const passing = results.filter(({ outcome }) => outcome.state === "pass");
  const sizes = await measure(codec, passing.map(({ outcome }) => ({ sha256: outcome.artifact.sha256, path: join(work, outcome.artifact.path) })));
  const table = {};
  for (const { item, lane, outcome } of results) {
    const row = (table[item.id] ??= {});
    row[lane.metric] = outcome.state === "pass"
      ? { state: "pass", size: sizes.get(outcome.artifact.sha256)[lane.metric], sha256: outcome.artifact.sha256.slice(0, 16) }
      : { state: outcome.state, detail: outcome.detail };
    if (retainExplanations) {
      const path = join(work, "out", binary.label, item.id, `${lane.id}.explain.txt`);
      row[lane.metric].explanation = { path: relative(work, path), sha256: sha256File(path) };
    }
  }
  return table;
}

async function prepareOracles(items, work, catalogEntries, verifyCatalog) {
  const cachePath = join(work, "oracle-cache.json");
  const cache = new Map(existsSync(cachePath) ? Object.entries(JSON.parse(readFileSync(cachePath, "utf8"))) : []);
  const cases = items.filter((item) => item.set === "cases");
  await pool(cases, 8, async (item) => {
    item.expected = await referenceStdout(item.js, cache);
  });
  if (verifyCatalog) {
    const byName = new Map();
    await pool(catalogEntries, 8, async (entry) => byName.set(entry.name, await referenceStdout(entry.js, cache)));
    const manifest = JSON.parse(readFileSync(join(casesRoot, "oracle-manifest.json"), "utf8"));
    const digest = catalogOracleDigest(catalogEntries, byName);
    if (manifest.cases !== catalogEntries.length || manifest.digest !== digest) {
      throw new Error(`comparison/cases catalog oracle drifted: manifest ${manifest.digest} (${manifest.cases} cases), observed ${digest} (${catalogEntries.length}). Review the reference programs, then run node comparison/cases/run.mjs --update-oracles.`);
    }
  }
  writeFileSync(cachePath, JSON.stringify(Object.fromEntries(cache)));
}

function materializeCatalog(items, work) {
  for (const item of items) {
    if (item.kind !== "catalog") continue;
    const directory = join(work, "src", item.id);
    mkdirSync(directory, { recursive: true });
    item.entry = join(directory, "main.lil");
    writeFileSync(item.entry, item.lil);
  }
}

// ---------------------------------------------------------------- verdicts

// Loss rows: one per (item, metric, bar) where the item's passing artifact is
// larger than the bar. Loss is in bytes of that metric.
export function lossRows(table, bars) {
  const rows = [];
  for (const [id, lanes] of Object.entries(table)) {
    const itemBars = bars[id] ?? {};
    for (const metric of METRICS) {
      const ours = lanes[metric];
      if (!ours || ours.state !== "pass") continue;
      for (const bar of BARS) {
        const value = itemBars[bar]?.[metric];
        if (!value || typeof value.size !== "number") continue;
        if (ours.size > value.size) rows.push({ id, set: id.split("/")[0], metric, bar, ours: ours.size, bar_size: value.size, loss: ours.size - value.size });
      }
    }
  }
  return rows;
}

export function failureRows(table) {
  const rows = [];
  for (const [id, lanes] of Object.entries(table)) {
    for (const metric of METRICS) {
      const lane = lanes[metric];
      if (lane && lane.state !== "pass") rows.push({ id, set: id.split("/")[0], metric, state: lane.state, detail: lane.detail });
    }
  }
  return rows;
}

export function countLosses(rows, ids) {
  const counts = {};
  for (const set of SETS) {
    counts[set] = {};
    for (const metric of METRICS) counts[set][metric] = Object.fromEntries(BARS.map((bar) => [bar, 0]));
  }
  for (const row of rows) {
    if (ids && !ids.has(row.id)) continue;
    counts[row.set][row.metric][row.bar] += 1;
  }
  return counts;
}

export function totals(table, bars, ids) {
  // Sum of our sizes and of each bar over the items where both exist, per metric.
  const sums = {};
  for (const [id, lanes] of Object.entries(table)) {
    if (ids && !ids.has(id)) continue;
    const set = id.split("/")[0];
    for (const metric of METRICS) {
      const ours = lanes[metric];
      if (!ours || ours.state !== "pass") continue;
      for (const bar of BARS) {
        const value = bars[id]?.[bar]?.[metric];
        if (!value || typeof value.size !== "number") continue;
        const slot = (((sums[set] ??= {})[metric] ??= {})[bar] ??= { items: 0, ours: 0, bar: 0 });
        slot.items += 1;
        slot.ours += ours.size;
        slot.bar += value.size;
      }
    }
  }
  return sums;
}

function covers(entry, predicate) {
  const patterns = Array.isArray(entry.items) ? entry.items : [entry.items];
  return patterns.some((pattern) => globMatcher(pattern)(predicate.id))
    && (!entry.metrics || entry.metrics.includes(predicate.metric))
    && (!entry.bars || predicate.bar === undefined || entry.bars.includes(predicate.bar));
}

export function validateLedger(ledger) {
  const problems = [];
  (ledger.entries ?? []).forEach((entry, index) => {
    const where = `ledger entry ${index + 1}`;
    if (!entry.items || (Array.isArray(entry.items) && entry.items.length === 0)) problems.push(`${where}: no items`);
    if (!["loss", "failure"].includes(entry.kind)) problems.push(`${where}: kind must be "loss" or "failure"`);
    if (typeof entry.owner !== "string" || !entry.owner.trim()) problems.push(`${where}: no owner (a plan task)`);
    if (typeof entry.reason !== "string" || !entry.reason.trim()) problems.push(`${where}: no reason`);
    for (const metric of entry.metrics ?? []) if (!METRICS.includes(metric)) problems.push(`${where}: unknown metric ${metric}`);
    for (const bar of entry.bars ?? []) if (!BARS.includes(bar)) problems.push(`${where}: unknown bar ${bar}`);
    if (entry.kind === "failure" && entry.bars) problems.push(`${where}: a failure entry names no bar`);
  });
  return problems;
}

// Which losses and failures the ledger covers, and which entries cover nothing.
export function applyLedger(ledger, losses, failures) {
  const entries = ledger.entries ?? [];
  const used = new Set();
  const cover = (row, kind) => {
    const index = entries.findIndex((entry) => entry.kind === kind && covers(entry, row));
    if (index >= 0) used.add(index);
    return index >= 0 ? entries[index] : null;
  };
  const unledgeredLosses = losses.filter((row) => !cover(row, "loss"));
  const unledgeredFailures = failures.filter((row) => !cover(row, "failure"));
  const stale = entries.map((entry, index) => ({ entry, index })).filter(({ index }) => !used.has(index));
  return { unledgeredLosses, unledgeredFailures, stale };
}

// Compare a run with the accepted baseline over the items both contain.
// Compare a run with the accepted baseline over the lanes (item, metric) that
// passed in both: a lane that starts compiling (a fixed refusal) may lose
// without that counting as growth, but its losses still need ledger entries.
export function compareWithBaseline(table, baselineTable, bars) {
  const common = new Set(Object.keys(table).filter((id) => baselineTable[id]));
  const bothPass = (row) => common.has(row.id)
    && table[row.id][row.metric]?.state === "pass"
    && baselineTable[row.id][row.metric]?.state === "pass";
  const now = lossRows(table, bars).filter(bothPass);
  const before = lossRows(baselineTable, bars).filter(bothPass);
  const key = (row) => `${row.id}\u0000${row.metric}\u0000${row.bar}`;
  const beforeByKey = new Map(before.map((row) => [key(row), row]));
  const nowByKey = new Map(now.map((row) => [key(row), row]));
  const grown = [];
  const shrunk = [];
  for (const [k, row] of nowByKey) {
    const old = beforeByKey.get(k);
    if (!old || row.loss > old.loss) grown.push({ ...row, before: old?.loss ?? 0 });
    else if (row.loss < old.loss) shrunk.push({ ...row, before: old.loss });
  }
  for (const [k, row] of beforeByKey) if (!nowByKey.has(k)) shrunk.push({ ...row, loss: 0, before: row.loss, ours: table[row.id]?.[row.metric]?.size ?? null });
  const countsNow = countLosses(now);
  const countsBefore = countLosses(before);
  const countChanges = [];
  for (const set of SETS) for (const metric of METRICS) for (const bar of BARS) {
    const a = countsBefore[set][metric][bar];
    const b = countsNow[set][metric][bar];
    if (a !== b) countChanges.push({ set, metric, bar, before: a, now: b });
  }
  const broken = [];
  const fixed = [];
  for (const id of common) for (const metric of METRICS) {
    const a = baselineTable[id][metric]?.state;
    const b = table[id][metric]?.state;
    if (a === "pass" && b && b !== "pass") broken.push({ id, metric, state: b, detail: table[id][metric].detail });
    if (a && a !== "pass" && b === "pass") fixed.push({ id, metric, was: a });
  }
  const added = Object.keys(table).filter((id) => !baselineTable[id]);
  return { grown, shrunk, countChanges, broken, fixed, added, countsNow, countsBefore };
}

// ---------------------------------------------------------------- reports

function formatCounts(counts, sums, sets) {
  const lines = ["| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |", "|---|---|---|---:|---:|---:|---:|"];
  for (const set of sets) for (const metric of METRICS) for (const bar of BARS) {
    const slot = sums?.[set]?.[metric]?.[bar];
    if (!slot) continue;
    lines.push(`| ${set} | ${metric} | ${bar} | ${counts[set][metric][bar]} | ${slot.items} | ${slot.ours} | ${slot.bar} |`);
  }
  return lines.join("\n");
}

function markdownReport(report) {
  const out = [`# Generic corpus ratchet`, "", `Compiler \`${report.compiler.sha256.slice(0, 16)}\`${report.compiler.label ? ` (${report.compiler.label})` : ""}, ${report.items} items, bars \`${report.barsSha256.slice(0, 16)}\`.`, ""];
  if (report.delivery) out.push(`Delivery: classic script, explicit \`${report.delivery.format}\`.`, "");
  out.push(formatCounts(report.counts, report.totals, report.sets), "");
  out.push(`Failures (compile, crash or oracle): ${report.failures.length}. Verdict: **${report.verdict}**.`, "");
  if (report.problems.length) out.push("## Blocking", "", ...report.problems.map((line) => `- ${line}`), "");
  if (report.improvements.length) out.push("## Improvements (tighten the baseline with --update-baseline)", "", ...report.improvements.map((line) => `- ${line}`), "");
  return `${out.join("\n")}\n`;
}

// ---------------------------------------------------------------- bars

async function loadMinifiers() {
  const { createRequire } = await import("node:module");
  const popular = join(repository, "benchmarks/popular");
  const require = createRequire(join(popular, "package.json"));
  const { minify: terserMinify } = require("terser");
  const esbuild = require("esbuild");
  const { minifySync: oxcMinify } = await import(pathToFileURL(join(popular, "node_modules/rolldown/dist/utils-index.mjs")));
  const versions = {
    terser: require("terser/package.json").version,
    esbuild: require("esbuild/package.json").version,
    rolldown: require("rolldown/package.json").version,
  };
  return { terserMinify, esbuild, oxcMinify, versions };
}

// The micro corpus's recipes (comparison/cases/recipes.mjs) on one app's
// JavaScript. A module graph is first bundled, unminified, into one script.
async function appCandidates(folder, minifiers) {
  const { baselineOptions } = await import(pathToFileURL(join(casesRoot, "recipes.mjs")));
  const sources = readdirSync(join(folder, "closure")).filter((name) => name.endsWith(".js")).sort();
  let source;
  if (sources.length === 1) {
    source = readFileSync(join(folder, "closure", sources[0]), "utf8");
  } else {
    const bundled = await minifiers.esbuild.build({
      entryPoints: [join(folder, "closure/main.js")], bundle: true, format: "iife", write: false, minify: false,
      target: "es2022", legalComments: "none", logLevel: "error",
    });
    source = bundled.outputFiles[0].text;
  }
  const clone = (value) => structuredClone(value);
  const candidates = [];
  const terser = await minifiers.terserMinify(source, clone(baselineOptions.terser));
  candidates.push({ tool: "terser", code: terser.code });
  const terserProperties = await minifiers.terserMinify(source, clone(baselineOptions["terser-properties"]));
  candidates.push({ tool: "terser-properties", code: terserProperties.code });
  const oxc = minifiers.oxcMinify("main.js", source, clone(baselineOptions.oxc));
  if (!oxc.errors?.length) candidates.push({ tool: "oxc", code: oxc.code });
  candidates.push({ tool: "esbuild-script", code: (await minifiers.esbuild.transform(source, clone(baselineOptions["esbuild-script"]))).code });
  candidates.push({ tool: "esbuild-iife", code: (await minifiers.esbuild.transform(source, clone(baselineOptions["esbuild-iife"]))).code });
  return candidates;
}

function smallestValid(candidates, metric) {
  const valid = candidates.filter((candidate) => candidate.valid && candidate.sizes);
  valid.sort((a, b) => a.sizes[metric] - b.sizes[metric] || a.sizes.raw - b.sizes.raw || (a.tool < b.tool ? -1 : 1));
  const best = valid[0];
  return best ? { size: best.sizes[metric], tool: best.tool, sha256: best.sha256.slice(0, 16) } : null;
}

function harnessBars(candidates, include) {
  const chosen = candidates.filter((candidate) => include(candidate.id ?? candidate.tool));
  return Object.fromEntries(METRICS.map((metric) => [metric, smallestValid(chosen.map((candidate) => ({
    tool: candidate.id ?? candidate.tool, valid: candidate.semanticValid === true, sizes: candidate.sizes, sha256: candidate.digest ?? "",
  })), metric)]));
}

async function runHarness(script, env, timeoutMs) {
  const result = await run(process.execPath, [script], { cwd: repository, env, timeoutMs });
  // Both harnesses exit 1 while any case misses its hard gate: that is the
  // state this ratchet replaces. A summary must still be written.
  return result;
}

async function refreshBars(options) {
  const work = resolve(options.work);
  mkdirSync(work, { recursive: true });
  const compiler = { ...pinBinary(options.compiler, join(work, "bin")), label: "harness" };
  const codec = pinBinary(options.codec, join(work, "bin"));
  const reference = { ...pinBinary(options.reference, join(work, "bin")), label: "old-route" };
  const env = { ...process.env, LILSCRIPT: compiler.path, LILSCRIPT_CODEC: codec.path };
  const minifiers = await loadMinifiers();
  const bars = {};
  const oracle = { traces: {} };

  process.stderr.write("bars: comparison/cases harness (Terser, Oxc, esbuild)\n");
  if (!options.reuseSummaries) await runHarness(join(casesRoot, "run.mjs"), env, 3 * 3600_000);
  const cases = JSON.parse(readFileSync(join(casesRoot, "summary.json"), "utf8"));
  if (cases.selectedBy !== "all") throw new Error("comparison/cases/summary.json is not a full run");
  for (const row of cases.rows) {
    const id = row.origin === "canonical" ? `cases/canonical/${row.name}` : catalogId(row.behavior, row.name);
    const candidates = Object.entries(row.baselineCandidates).map(([tool, candidate]) => ({ id: tool, ...candidate }));
    bars[id] = { competitor: harnessBars(candidates, () => true) };
  }

  process.stderr.write("bars: comparison/algorithms harness (competitors and Closure ADVANCED)\n");
  if (!options.reuseSummaries) await runHarness(join(algorithmsRoot, "run.mjs"), env, 3600_000);
  const algorithms = JSON.parse(readFileSync(join(algorithmsRoot, "summary.json"), "utf8"));
  for (const row of algorithms.rows) {
    const id = `algorithms/${row.id}`;
    bars[id] = {
      competitor: harnessBars(row.baselineCandidates, (tool) => !tool.startsWith("closure")),
      closure: harnessBars(row.baselineCandidates, (tool) => tool.startsWith("closure")),
    };
    oracle.traces[id] = Object.fromEntries(row.referenceVectorResults.map((result) => [result.vector, result.hostAccessTrace]));
  }

  process.stderr.write("bars: comparison/apps (competitors on the Closure input; committed Closure artifacts)\n");
  const { items } = await loadCorpus(SETS);
  const appDir = join(work, "bars", "apps");
  for (const item of items.filter((row) => row.set === "apps")) {
    const name = basename(item.folder);
    const candidates = await appCandidates(item.folder, minifiers);
    candidates.push({ tool: "closure-advanced", code: readFileSync(join(repository, "comparison/artifacts", name, "closure-advanced.js"), "utf8") });
    for (const candidate of candidates) {
      const path = join(appDir, name, `${candidate.tool}.js`);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, candidate.code);
      candidate.path = path;
      candidate.sha256 = sha256(candidate.code);
      const result = await executeScript(candidate.code, { cwd: item.folder, timeoutMs: 10_000 });
      candidate.valid = result.status === 0 && result.stdout === item.expected;
    }
    const sizes = await measure(codec, candidates.map((candidate) => ({ sha256: candidate.sha256, path: candidate.path })));
    for (const candidate of candidates) candidate.sizes = sizes.get(candidate.sha256);
    const competitors = candidates.filter((candidate) => !candidate.tool.startsWith("closure"));
    const closure = candidates.filter((candidate) => candidate.tool.startsWith("closure"));
    bars[item.id] = {
      competitor: Object.fromEntries(METRICS.map((metric) => [metric, smallestValid(competitors, metric)])),
      closure: Object.fromEntries(METRICS.map((metric) => [metric, smallestValid(closure, metric)])),
    };
  }

  process.stderr.write("bars: the frozen old route on every item\n");
  materializeCatalog(items, work);
  await prepareOracles(items, work, [], false);
  const old = await compileCorpus({
    binary: reference, extraArgs: ["--backend", "legacy"], configFor: () => OLD_ROUTE_CONFIGS,
    items, work, jobs: options.jobs, codec, oracle,
  });
  for (const item of items) {
    const lanes = old[item.id];
    bars[item.id] ??= {};
    bars[item.id].old = Object.fromEntries(METRICS.map((metric) => [metric, lanes[metric].state === "pass"
      ? { size: lanes[metric].size, sha256: lanes[metric].sha256 }
      : { refused: lanes[metric].state, detail: lanes[metric].detail }]));
  }
  for (const item of items) for (const bar of BARS) if (bars[item.id]?.[bar] && Object.values(bars[item.id][bar]).every((value) => value === null)) delete bars[item.id][bar];
  const document = {
    schema: 1,
    about: "Bars of the generic corpus ratchet (scripts/ratchet.mjs, plan task M2.13). Refreshed only as a scheduled re-baseline event (BC3): competitor versions, Closure and the frozen old route are pinned here by what produced them. competitor = the smallest valid Terser, Oxc or esbuild artifact (the algorithms add their harness's bundler lanes); closure = Closure ADVANCED (apps: comparison/artifacts/*/closure-advanced.js; algorithms: their harness); old = the reference binary with --backend legacy and tests/ratchet/old-route/*.toml. A size is in its own metric; sha256 is a 16-hex prefix of the artifact digest. oracles.traces holds the algorithms' reference host-access traces.",
    refreshed: new Date().toISOString().slice(0, 10),
    provenance: {
      harnessCompiler: { sha256: compiler.sha256, version: compiler.version },
      reference: { source: reference.source, sha256: reference.sha256, args: ["--backend", "legacy"], configs: OLD_ROUTE_CONFIGS },
      codec: { sha256: codec.sha256 },
      node: process.version,
      tools: { ...minifiers.versions, closure: algorithms.toolVersions?.googleClosureCompiler ?? algorithms.toolVersions ?? null, casesHarness: cases.toolVersions ?? null },
    },
    oracles: oracle,
    items: Object.fromEntries(Object.entries(bars).sort(([a], [b]) => (a < b ? -1 : 1))),
  };
  const target = resolve(options.bars);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, `${JSON.stringify(document, null, 1)}\n`);
  process.stderr.write(`wrote ${relative(repository, target)}: ${Object.keys(bars).length} items\n`);
}

// ---------------------------------------------------------------- check

// Historical baselines measured bare classic scripts. Pin that container
// explicitly so a compiler-default change cannot silently change the gate.
export function deliveryProblem(format, baseline) {
  if (!["bare", "iife"].includes(format)) throw new Error(`unknown script format ${format}; use bare or iife`);
  if (!baseline) return null;
  const previous = baseline.delivery ?? { target: "js", format: "bare" };
  return previous.target !== "js" || previous.format !== format
    ? `delivery contract changed: ${previous.target}/${previous.format} -> js/${format}; qualify with a separate baseline`
    : null;
}

export function configurationSnapshot(sets, directory) {
  return sets.flatMap((set) => LANES.map((lane) => {
    const path = resolve(repository, directory ?? CONFIGS[set], `${lane.id}.toml`);
    return { set, lane: lane.id, path, sha256: sha256File(path) };
  }));
}

export function changedConfigurations(snapshot) {
  return snapshot.filter((file) => !existsSync(file.path) || sha256File(file.path) !== file.sha256)
    .map((file) => `configuration changed during measurement: ${file.set}/${file.lane} ${file.path}`);
}

export function configurationOverrideProblem(directory, baselinePath, update) {
  if (!directory || !update) return null;
  const canonical = (path) => existsSync(path) ? realpathSync(path) : resolve(path);
  const ordinary = resolve(repository, DEFAULTS.baseline);
  const left = existsSync(baselinePath) ? statSync(baselinePath) : null;
  const right = existsSync(ordinary) ? statSync(ordinary) : null;
  const sameFile = left && right && left.dev === right.dev && left.ino === right.ino;
  return sameFile || canonical(baselinePath) === canonical(ordinary)
    ? "experimental --config-dir cannot update the ordinary baseline; use a separate --baseline file"
    : null;
}

export function validateObjectivePolicy(receipt, lane) {
  const expected = { raw: "Raw", gzip: "Gzip", brotli: "Brotli" }[lane];
  const actual = receipt?.policy?.objective?.codec;
  if (!expected || actual !== expected) throw new Error(`configuration for ${lane} resolves objective ${actual ?? "missing"}; expected ${expected}`);
  const canonical = { gzip: { level: 9, window: 15 }, brotli: { quality: 11, window: 22, mode: "generic" } }[lane];
  const settings = receipt?.policy?.objective?.codec_settings?.[lane];
  if (canonical && Object.entries(canonical).some(([key, value]) => settings?.[key] !== value)) {
    throw new Error(`configuration for ${lane} must use the ratchet's canonical encoder settings: ${JSON.stringify(canonical)}`);
  }
}

export async function runRatchet(options) {
  const started = Date.now();
  const work = resolve(options.work);
  mkdirSync(work, { recursive: true });
  const barsDocument = JSON.parse(readFileSync(resolve(options.bars), "utf8"));
  const barsSha256 = sha256File(resolve(options.bars));
  const bars = barsDocument.items;
  const ledger = JSON.parse(readFileSync(resolve(options.ledger), "utf8"));
  const ledgerProblems = validateLedger(ledger);
  if (ledgerProblems.length) throw new Error(`the ratchet ledger is invalid:\n${ledgerProblems.join("\n")}`);
  const baselinePath = resolve(options.baseline);
  const configDir = options.configDir ? resolve(options.configDir) : undefined;
  const overrideProblem = configurationOverrideProblem(configDir, baselinePath, options.updateBaseline);
  if (overrideProblem) throw new Error(overrideProblem);
  const baseline = existsSync(baselinePath) ? JSON.parse(readFileSync(baselinePath, "utf8")) : null;
  const format = options.format ?? "bare";
  const contractProblem = deliveryProblem(format, baseline);

  const sets = options.sets;
  const configurations = configurationSnapshot(sets, configDir);
  const { items: corpus, catalogEntries } = await loadCorpus(sets);
  const items = selectItems(corpus, options.filter);
  if (items.length === 0) throw new Error("no corpus item matches the selection");
  const fullCatalog = sets.includes("cases") && !options.filter;
  materializeCatalog(items, work);
  await prepareOracles(items, work, catalogEntries, fullCatalog);

  const compiler = { ...pinBinary(options.compiler, join(work, "bin")), label: "compiler" };
  const codec = pinBinary(options.codec, join(work, "bin"));
  for (const file of configurations) {
    const item = items.find((item) => item.set === file.set);
    if (!item) continue;
    const result = await run(compiler.path, [item.entry, "--config", file.path, "--target", "js", "--format", format, "--print-policy"], { cwd: dirname(item.entry), timeoutMs: 30_000, env: compilerEnvironment });
    if (result.status !== 0) throw new Error(`cannot resolve ${file.set}/${file.lane}: ${keyLine(result.stderr)}`);
    file.resolved = JSON.parse(result.stdout);
    validateObjectivePolicy(file.resolved, file.lane);
  }
  const extraArgs = ["--format", format, ...(options.retainExplanations ? ["--explain", "json"] : [])];
  const table = await compileCorpus({
    binary: compiler, extraArgs, configFor: (item) => configDir ?? CONFIGS[item.set],
    items, work, jobs: options.jobs, codec, oracle: barsDocument.oracles ?? {},
    retainExplanations: options.retainExplanations,
  });

  const ids = new Set(items.map((item) => item.id));
  const losses = lossRows(table, bars);
  const failures = failureRows(table);
  const counts = countLosses(losses, ids);
  const sums = totals(table, bars, ids);
  const { unledgeredLosses, unledgeredFailures, stale } = applyLedger(ledger, losses, failures);
  const problems = changedConfigurations(configurations);
  if (contractProblem) problems.push(contractProblem);
  const improvements = [];

  for (const row of unledgeredLosses) problems.push(`unledgered loss: ${row.id} ${row.metric} ${row.ours} > ${row.bar} ${row.bar_size} (+${row.loss}); add a ledger entry with an owner`);
  for (const row of unledgeredFailures) problems.push(`unledgered failure: ${row.id} ${row.metric} ${row.state}: ${row.detail ?? ""}`);

  let comparison = null;
  if (!baseline) {
    problems.push(`no baseline at ${relative(repository, baselinePath)}: run with --update-baseline to record one`);
  } else {
    if (baseline.barsSha256 !== barsSha256) {
      problems.push(`the bars changed since the baseline (${baseline.barsSha256?.slice(0, 16)} -> ${barsSha256.slice(0, 16)}): re-baseline deliberately with --update-baseline (a scheduled event, BC3)`);
    }
    comparison = compareWithBaseline(table, baseline.items, bars);
    for (const change of comparison.countChanges) {
      const line = `${change.set} ${change.metric} losses to ${change.bar}: ${change.before} -> ${change.now}`;
      if (change.now > change.before) problems.push(`loss count grew: ${line}`);
      else improvements.push(`loss count fell: ${line}`);
    }
    for (const row of comparison.grown) problems.push(`loss grew: ${row.id} ${row.metric} vs ${row.bar}: +${row.before} -> +${row.loss} (ours ${row.ours}, bar ${row.bar_size})`);
    for (const row of comparison.shrunk) improvements.push(`loss fell: ${row.id} ${row.metric} vs ${row.bar}: +${row.before} -> +${row.loss}`);
    for (const row of comparison.broken) problems.push(`regressed: ${row.id} ${row.metric} passed in the baseline and is now ${row.state}: ${row.detail ?? ""}`);
    for (const row of comparison.fixed) improvements.push(`now passes: ${row.id} ${row.metric} (was ${row.was})`);
    for (const id of comparison.added) improvements.push(`new item, not in the baseline: ${id}`);
  }
  // A partial run cannot tell a stale entry from one covering unselected items.
  if (!options.filter && sets.length === SETS.length) {
    for (const { entry, index } of stale) improvements.push(`ledger entry ${index + 1} covers nothing (remove it): ${JSON.stringify(entry.items).slice(0, 120)}`);
  }

  const verdict = problems.length === 0 ? "pass" : "fail";
  const report = {
    schema: 1,
    started: new Date(started).toISOString(),
    seconds: Math.round((Date.now() - started) / 1000),
    compiler: { source: compiler.source, sha256: compiler.sha256, version: compiler.version, label: options.label ?? null },
    codec: { sha256: codec.sha256 },
    delivery: { target: "js", format },
    configurations,
    configurationOverride: configDir ?? null,
    explanations: options.retainExplanations ?? false,
    sets,
    filter: options.filter ?? null,
    items: items.length,
    barsSha256,
    counts,
    totals: sums,
    losses,
    failures,
    problems,
    improvements,
    verdict,
    table,
  };
  if (options.json) writeFileSync(resolve(options.json), `${JSON.stringify(report, null, 1)}\n`);
  if (options.markdown) writeFileSync(resolve(options.markdown), markdownReport(report));

  process.stdout.write(`${formatCounts(counts, sums, sets)}\n\n`);
  process.stdout.write(`${items.length} items, ${failures.length} failing lanes, ${losses.length} loss rows, ${report.seconds} s.\n`);
  if (problems.length) process.stdout.write(`\nBlocking (${problems.length}):\n${problems.slice(0, options.verbose ? problems.length : 40).map((line) => `  ${line}`).join("\n")}\n`);
  if (improvements.length) process.stdout.write(`\nImprovements (${improvements.length}; tighten with --update-baseline):\n${improvements.slice(0, options.verbose ? improvements.length : 40).map((line) => `  ${line}`).join("\n")}\n`);

  if (options.updateBaseline) {
    const growth = (line) => line.startsWith("loss grew:") || line.startsWith("loss count grew:");
    // Growth is accepted only for an overall win: no corpus total (ours,
    // per metric) above the baseline's.
    const grownTotals = [];
    if (options.acceptGrowth && baseline) {
      for (const [set, metrics] of Object.entries(sums)) {
        for (const [metric, bars] of Object.entries(metrics)) {
          const now = Object.values(bars)[0]?.ours;
          const before = Object.values(baseline.totals?.[set]?.[metric] ?? {})[0]?.ours;
          if (now !== undefined && before !== undefined && now > before) grownTotals.push(`${set} ${metric}: ${before} -> ${now}`);
        }
      }
    }
    const acceptable = options.acceptGrowth && grownTotals.length === 0;
    const refusing = problems.filter((line) => !line.startsWith("no baseline") && !line.startsWith("the bars changed") && !(acceptable && growth(line)));
    if (options.acceptGrowth && grownTotals.length) {
      process.stdout.write(`\n--accept-growth refused: a corpus total grew:\n${grownTotals.map((line) => `  ${line}`).join("\n")}\n`);
    }
    if (acceptable) {
      const accepted = problems.filter(growth);
      if (accepted.length) process.stdout.write(`\n--accept-growth accepts (${accepted.length}):\n${accepted.map((line) => `  ${line}`).join("\n")}\n`);
    }
    if (refusing.length) {
      process.stdout.write("\n--update-baseline refused: the run has blocking problems other than a missing or re-barred baseline.\n");
    } else if (options.filter || sets.length !== SETS.length) {
      process.stdout.write("\n--update-baseline refused: record a baseline from a full run (every corpus, no --filter).\n");
    } else {
      const document = {
        schema: 1,
        about: "Accepted state of the generic corpus ratchet (scripts/ratchet.mjs, plan task M2.13): each item's artifact size per objective lane, or its failure state. Loss counts and losses are derived against bars.json (barsSha256). Written only by --update-baseline, which refuses while anything regresses; see docs/testing.md.",
        recorded: new Date().toISOString().slice(0, 10),
        compiler: report.compiler,
        delivery: report.delivery,
        configurations,
        configurationOverride: report.configurationOverride,
        barsSha256,
        counts,
        totals: sums,
        items: Object.fromEntries(Object.entries(table).sort(([a], [b]) => (a < b ? -1 : 1)).map(([id, lanes]) => [id, Object.fromEntries(METRICS.map((metric) => [metric, lanes[metric].state === "pass" ? { state: "pass", size: lanes[metric].size } : { state: lanes[metric].state }]))])),
      };
      writeFileSync(baselinePath, `${JSON.stringify(document, null, 1)}\n`);
      process.stdout.write(`\nwrote ${relative(repository, baselinePath)}\n`);
      return 0;
    }
  }
  return verdict === "pass" ? 0 : 1;
}

function defaultCodec(compiler) {
  for (const candidate of [join(dirname(resolve(compiler)), "lilscript-codec"), join(repository, "target/release/lilscript-codec")]) {
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

async function main() {
  const { values } = parseArgs({
    options: {
      compiler: { type: "string" },
      codec: { type: "string" },
      reference: { type: "string" },
      sets: { type: "string" },
      filter: { type: "string" },
      format: { type: "string" },
      "config-dir": { type: "string" },
      "retain-explanations": { type: "boolean" },
      jobs: { type: "string" },
      work: { type: "string" },
      json: { type: "string" },
      markdown: { type: "string" },
      bars: { type: "string" },
      baseline: { type: "string" },
      ledger: { type: "string" },
      label: { type: "string" },
      "refresh-bars": { type: "boolean" },
      "reuse-summaries": { type: "boolean" },
      "update-baseline": { type: "boolean" },
      "accept-growth": { type: "boolean" },
      verbose: { type: "boolean" },
      help: { type: "boolean" },
    },
  });
  if (values.help || !values.compiler) {
    process.stderr.write("usage: node scripts/ratchet.mjs --compiler <lilscript> [--codec PATH] [--sets cases,apps,algorithms] [--filter <id|glob>,...] [--format bare|iife] [--config-dir DIR] [--retain-explanations] [--jobs N] [--work DIR] [--json FILE] [--markdown FILE] [--bars FILE] [--baseline FILE] [--ledger FILE] [--update-baseline] [--verbose]\n       node scripts/ratchet.mjs --refresh-bars --compiler <lilscript> --reference <old-route lilscript> [--codec PATH] [--jobs N]\n");
    return values.help ? 0 : 2;
  }
  const codec = values.codec ?? defaultCodec(values.compiler);
  if (!codec) throw new Error("no lilscript-codec beside the compiler; pass --codec");
  const sets = (values.sets ?? SETS.join(",")).split(",").map((set) => set.trim()).filter(Boolean);
  for (const set of sets) if (!SETS.includes(set)) throw new Error(`unknown corpus ${set}; one of ${SETS.join(", ")}`);
  const options = {
    compiler: values.compiler,
    codec,
    reference: values.reference,
    sets,
    filter: values.filter,
    format: values.format ?? "bare",
    configDir: values["config-dir"],
    retainExplanations: values["retain-explanations"] ?? false,
    jobs: Math.max(1, Number(values.jobs ?? Math.min(3, Math.max(1, availableParallelism() - 2)))),
    work: values.work ?? join(repository, "target/verify/ratchet"),
    json: values.json,
    markdown: values.markdown,
    bars: values.bars ?? join(repository, DEFAULTS.bars),
    baseline: values.baseline ?? join(repository, DEFAULTS.baseline),
    ledger: values.ledger ?? join(repository, DEFAULTS.ledger),
    label: values.label,
    updateBaseline: values["update-baseline"] ?? false,
    acceptGrowth: values["accept-growth"] ?? false,
    reuseSummaries: values["reuse-summaries"] ?? false,
    verbose: values.verbose ?? false,
  };
  if (values["refresh-bars"]) {
    if (!values.reference) throw new Error("--refresh-bars needs --reference <the frozen old-route binary>");
    await refreshBars(options);
    return 0;
  }
  return runRatchet(options);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().then((code) => process.exit(code), (error) => {
    process.stderr.write(`${error.stack ?? error}\n`);
    process.exit(2);
  });
}
