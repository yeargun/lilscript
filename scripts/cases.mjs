#!/usr/bin/env node
// The case runner (plan task M2.2): every corpus case in every lane.
//
//   node scripts/cases.mjs --compiler <lilscript> [--lanes <spec>] [--filter <spec>]
//        [--json out.json] [--compare previous.json] [--jobs N] [--work DIR]
//        [--cc /usr/bin/cc] [--codec <lilscript-codec>|none] [--ledger FILE]
//        [--node <node>] [--timeout SECONDS]
//
// A case is a `.lil` entry with an expected-stdout `.out` beside it, found under
// tests/cases (recursively, skipping multi-module folders) and tests/modules.
// Lanes are {formation-only, production} x {brotli, gzip, raw} x {script,
// module, c}. Every lane compiles with `[javascript] strip_console = false`:
// print is the observation channel and the `.out` file is the oracle.
//
// Each (case, lane) ends in exactly one state:
//   pass          compiled, ran, exit 0, stdout identical to the `.out` file
//   masked        the case uses a feature the target does not have (below)
//   refused       the compiler exited with a diagnostic
//   compiler-crash  the compiler panicked or was killed
//   cc-rejected   the C compiler rejected the emitted C
//   crashed       the program exited non-zero, was killed or timed out
//   wrong-output  the program exited 0 but printed something else
// Every state but pass and masked is a failure. A failure listed in the
// expected-failure ledger (tests/cases/expected-failures.json) is reported as
// ledgered; any other failure makes the exit code 1. Ledger entries whose
// lanes now pass are reported for removal.
//
// See docs/testing.md.
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import {
  firstDifference, gitIdentity, globMatcher, headLines, pinBinary, pool, repository, run, sha256, sha256File,
} from "./lib/verify-util.mjs";

export const MODES = ["formation-only", "production"];
export const CODECS = ["brotli", "gzip", "raw"];
export const TARGETS = {
  script: { flag: "js", extension: "js", javascript: true },
  module: { flag: "js-module", extension: "mjs", javascript: true },
  c: { flag: "c", extension: "c", javascript: false },
};
export const LANES = MODES.flatMap((mode) => CODECS.flatMap((codec) => Object.keys(TARGETS).map((target) => ({
  id: `${mode}/${codec}/${target}`, mode, codec, target, parts: [mode, codec, target],
}))));
const JAVASCRIPT = ["script", "module"];

// The per-target feature mask, declared once. A case using a feature is run
// only on the listed targets and reported as masked on the others. Detection is
// lexical, on source with comments and string text removed. `export` counts
// only in the entry module (an imported module's exports are internal
// linkage); every other feature counts in any module the entry imports.
export const FEATURES = [
  { id: "host-prelude", targets: JAVASCRIPT, why: "a .host.js prelude defines externs in a JavaScript realm" },
  { id: "module-probe", targets: ["module"], why: "a .module-probe.mjs imports the ES module's exports" },
  { id: "JsValue", targets: JAVASCRIPT, pattern: /\bJsValue\b/, why: "language-v0.1: JsValue is JavaScript-only" },
  { id: "extern", targets: JAVASCRIPT, pattern: /\bextern\b/, why: "language-v0.1: C rejects host declarations", lifts: "M11.3 (externs per target)" },
  { id: "export", targets: JAVASCRIPT, pattern: /\bexport\b/, entryOnly: true, why: "the entry's exports are a module ABI; C has none yet", lifts: "M11.8 (a C library ABI)" },
  { id: "JS namespace", targets: JAVASCRIPT, pattern: /\bJS\./, why: "language-v0.1: C rejects the JS.* operations" },
  { id: "async", targets: JAVASCRIPT, pattern: /\b(?:async|await|Task)\b/, why: "language-v0.1: native rejects async functions and tasks", lifts: "M11.6 (portable subset)" },
  { id: "exceptions", targets: JAVASCRIPT, pattern: /\b(?:throw|try)\b/, why: "language-v0.1: native rejects exceptions", lifts: "M11.6 (portable subset)" },
  { id: "Regex", targets: JAVASCRIPT, pattern: /\bRegex\b/, why: "language-v0.1: native rejects Regex", lifts: "M11.6 (portable subset)" },
  { id: "generator", targets: JAVASCRIPT, pattern: /\bgenerator\b/, why: "language-v0.1: native rejects generators", lifts: "M11.6 (portable subset)" },
  { id: "object literal", targets: JAVASCRIPT, pattern: /\bobject\s*\{/, why: "language-v0.1: object {} is JavaScript-only" },
  { id: "JSON.parse", targets: JAVASCRIPT, pattern: /\bJSON\.parse\b/, why: "language-v0.1: JSON.parse returns JsValue" },
];

// Used only when the compiler cannot print its policy.
const FALLBACK_TACTICS = [
  "dead-code-elimination", "constant-folding", "inlining", "scalar-replacement", "call-specialization",
  "helper-sharing", "target-compaction", "identifier-mangling", "property-mangling", "string-pooling",
  "string-array-packing", "startup-reconstruction", "recurring-reconstruction", "naming-search",
];
const CC_FLAGS = ["-std=c11", "-O2", "-fno-fast-math", "-ffp-contract=off"];
const FAILURES = ["refused", "compiler-crash", "cc-rejected", "crashed", "wrong-output"];

// ---------------------------------------------------------------- sources

// Source text with comments removed and string/template text blanked, so a
// word in a comment or a printed string is never mistaken for a feature.
export function codeOnly(text) {
  let out = "";
  const templates = []; // brace depth of each open `${` expression
  let index = 0;
  const readString = (quote) => {
    index += 1;
    while (index < text.length && text[index] !== quote && text[index] !== "\n") index += text[index] === "\\" ? 2 : 1;
    index += 1;
    out += `${quote}${quote}`;
  };
  const readTemplate = () => {
    while (index < text.length) {
      if (text[index] === "\\") { index += 2; continue; }
      if (text[index] === "`") { index += 1; out += "`"; return; }
      if (text[index] === "$" && text[index + 1] === "{") { index += 2; out += "${"; templates.push(0); return; }
      index += 1;
    }
  };
  while (index < text.length) {
    const char = text[index];
    const next = text[index + 1];
    if (char === "/" && next === "/") { while (index < text.length && text[index] !== "\n") index += 1; continue; }
    if (char === "/" && next === "*") { const end = text.indexOf("*/", index + 2); index = end < 0 ? text.length : end + 2; out += " "; continue; }
    if (char === '"' || char === "'") { readString(char); continue; }
    if (char === "`") { out += "`"; index += 1; readTemplate(); continue; }
    if (templates.length) {
      if (char === "{") templates[templates.length - 1] += 1;
      if (char === "}") {
        if (templates[templates.length - 1] === 0) { templates.pop(); out += "}"; index += 1; readTemplate(); continue; }
        templates[templates.length - 1] -= 1;
      }
    }
    out += char;
    index += 1;
  }
  return out;
}

// The entry plus every module it reaches through relative imports.
function moduleGraph(entry) {
  const seen = new Set();
  const visit = (file) => {
    if (seen.has(file) || !existsSync(file)) return;
    seen.add(file);
    const code = readFileSync(file, "utf8").replace(/\/\/.*$/gm, "");
    for (const match of code.matchAll(/\b(?:import|export)\b[^;"']*?["'](\.{1,2}\/[^"']+)["']/g)) {
      const target = resolve(dirname(file), match[1]);
      visit(existsSync(`${target}.lil`) ? `${target}.lil` : target);
    }
  };
  visit(entry);
  return [...seen];
}

export function discoverCases(roots = [join(repository, "tests/cases"), join(repository, "tests/modules")]) {
  const cases = [];
  const walk = (directory) => {
    const names = readdirSync(directory).sort();
    for (const name of names) {
      const path = join(directory, name);
      if (statSync(path).isDirectory()) {
        // A folder named after a case holds that case's other modules.
        if (!names.includes(`${name}.lil`)) walk(path);
        continue;
      }
      if (!name.endsWith(".lil")) continue;
      const stem = name.slice(0, -4);
      const base = join(directory, stem);
      if (!existsSync(`${base}.out`)) continue;
      cases.push(describeCase(base));
    }
  };
  for (const root of roots) if (existsSync(root)) walk(root);
  return cases;
}

function describeCase(base) {
  const source = `${base}.lil`;
  const optional = (suffix) => (existsSync(`${base}${suffix}`) ? `${base}${suffix}` : null);
  const text = readFileSync(source, "utf8");
  const modules = moduleGraph(source);
  const entryCode = codeOnly(text);
  const allCode = modules.map((file) => codeOnly(readFileSync(file, "utf8"))).join("\n");
  const item = {
    id: relative(join(repository, "tests"), base),
    source,
    modules: modules.filter((file) => file !== source),
    expected: `${base}.out`,
    host: optional(".host.js"),
    toml: optional(".toml"),
    probe: optional(".module-probe.mjs"),
    // The test ran its script with "use strict"; prepended.
    strict: text.split("\n").slice(0, 8).some((line) => /^\s*\/\/\s*harness:\s*"use strict"/.test(line)),
  };
  item.features = FEATURES.filter((feature) => {
    if (feature.id === "host-prelude") return item.host !== null;
    if (feature.id === "module-probe") return item.probe !== null;
    return feature.pattern.test(feature.entryOnly ? entryCode : allCode);
  }).map((feature) => feature.id);
  const inputs = [source, ...item.modules, item.expected, item.host, item.toml, item.probe].filter(Boolean);
  item.digest = sha256(inputs.map((file) => `${relative(repository, file)}\0${sha256File(file)}`).join("\n"));
  return item;
}

export function maskFor(item, target) {
  return item.features.filter((id) => !FEATURES.find((feature) => feature.id === id).targets.includes(target));
}

// ---------------------------------------------------------------- lanes

// Comma-separated items, unioned. An item `a/b/c` matches mode/codec/target
// component-wise (each may use `*`); a single word matches any component.
export function selectLanes(spec) {
  if (!spec || spec === "all") return LANES;
  const items = spec.split(",").map((item) => item.trim()).filter(Boolean);
  const matches = (lane, item) => {
    if (item.includes("/")) {
      const parts = item.split("/");
      if (parts.length !== 3) throw new Error(`lane pattern ${item} must be mode/codec/target`);
      return parts.every((part, index) => globMatcher(part)(lane.parts[index]));
    }
    return lane.parts.some((part) => globMatcher(item)(part));
  };
  for (const item of items) {
    if (!LANES.some((lane) => matches(lane, item))) throw new Error(`lane pattern ${item} matches no lane`);
  }
  return LANES.filter((lane) => items.some((item) => matches(lane, item)));
}

// ---------------------------------------------------------------- configuration

// A small TOML reader for the runner's own merge: table headers and
// `key = value` lines (values kept verbatim, multi-line arrays joined).
export function parseTomlTables(text) {
  const tables = new Map([["", new Map()]]);
  let current = "";
  const lines = text.split("\n");
  const stripComment = (line) => {
    let quote = null;
    for (let index = 0; index < line.length; index += 1) {
      const char = line[index];
      if (quote) { if (char === "\\" && quote === '"') index += 1; else if (char === quote) quote = null; }
      else if (char === '"' || char === "'") quote = char;
      else if (char === "#") return line.slice(0, index);
    }
    return line;
  };
  const depth = (value) => [...value.replace(/"(?:\\.|[^"\\])*"|'[^']*'/g, "")].reduce((sum, char) => sum + (char === "[" || char === "{" ? 1 : char === "]" || char === "}" ? -1 : 0), 0);
  for (let index = 0; index < lines.length; index += 1) {
    const line = stripComment(lines[index]).trim();
    if (!line) continue;
    if (line.startsWith("[[")) throw new Error("arrays of tables are not supported in case configuration");
    const header = /^\[\s*([^\]]+?)\s*\]$/.exec(line);
    if (header) {
      current = header[1];
      if (!tables.has(current)) tables.set(current, new Map());
      continue;
    }
    const equals = line.indexOf("=");
    if (equals < 0) throw new Error(`unreadable configuration line: ${line}`);
    let value = line.slice(equals + 1).trim();
    while (depth(value) > 0 && index + 1 < lines.length) value += `\n${stripComment(lines[++index]).trim()}`;
    tables.get(current).set(line.slice(0, equals).trim(), value);
  }
  return tables;
}

export function renderToml(tables) {
  let text = "";
  for (const [name, entries] of tables) {
    if (!entries.size) continue;
    if (name) text += `[${name}]\n`;
    for (const [key, value] of entries) text += `${key} = ${value}\n`;
    text += "\n";
  }
  return text;
}

function laneTables(lane, tactics) {
  const javascript = new Map([["strip_console", "false"], ["cost_model", JSON.stringify(lane.codec)]]);
  const tables = new Map([["", new Map()], ["javascript", javascript]]);
  if (lane.mode === "formation-only") {
    javascript.set("candidate_search", '"off"');
    tables.set("policy.tactics", new Map(tactics.map((id) => [id, '"off"'])));
  }
  return tables;
}

// Case keys are merged into the lane's tables (one `[javascript]` table,
// never a second); a case key overrides the lane's value for that key, except
// that no case may strip print, the observation channel.
export function composeConfig(lane, tactics, caseToml) {
  const tables = laneTables(lane, tactics);
  if (caseToml) {
    for (const [name, entries] of parseTomlTables(caseToml)) {
      if (!tables.has(name)) tables.set(name, new Map());
      for (const [key, value] of entries) {
        if (name === "javascript" && key === "strip_console" && value !== "false") throw new Error("a case may not set strip_console");
        tables.get(name).set(key, value);
      }
    }
  }
  return renderToml(tables);
}

// ---------------------------------------------------------------- ledger

export function loadLedger(path) {
  if (!path || !existsSync(path)) return { path: path ?? null, sha256: null, entries: [] };
  const document = JSON.parse(readFileSync(path, "utf8"));
  const entries = (document.entries ?? []).map((entry, index) => {
    for (const field of ["reason", "owner"]) {
      if (typeof entry[field] !== "string" || !entry[field].trim()) throw new Error(`${path}: entry ${index} needs a non-empty "${field}"`);
    }
    const patterns = [entry.case].flat();
    if (!patterns.length || patterns.some((pattern) => typeof pattern !== "string" || !pattern.trim())) throw new Error(`${path}: entry ${index} needs "case" (an id, a glob, or a list of them)`);
    const lanes = Array.isArray(entry.lanes) ? entry.lanes.join(",") : (entry.lanes ?? "all");
    const selected = new Set(selectLanes(lanes).map((lane) => lane.id));
    const matchers = patterns.map(globMatcher);
    const matchCase = (caseId) => matchers.some((match) => match(caseId));
    return { ...entry, index, patterns, matchCase, matches: (caseId, laneId) => matchCase(caseId) && selected.has(laneId) };
  });
  return { path, sha256: sha256File(path), entries };
}

// ---------------------------------------------------------------- running

function compilerEnvironment() {
  const env = {};
  const scrubbed = [];
  for (const [key, value] of Object.entries(process.env)) {
    // Environment knobs change compiler behaviour; a lane is its policy only.
    if (key.startsWith("LILSCRIPT_")) scrubbed.push(key);
    else env[key] = value;
  }
  return { env, scrubbed };
}

function keyLine(stderr) {
  const lines = stderr.split("\n").map((line) => line.trim()).filter(Boolean);
  return (lines.find((line) => /\b\w*Error\b|panicked|error:/.test(line)) ?? lines[0] ?? "").slice(0, 300);
}

async function measureCodec(codec, files) {
  const sizes = new Map();
  if (!codec) return sizes;
  const unique = [...new Map(files.map((file) => [file.sha256, file.path])).entries()];
  for (let start = 0; start < unique.length; start += 200) {
    const batch = unique.slice(start, start + 200);
    const result = await run(codec.path, ["--json", ...batch.map(([, path]) => path)], { timeoutMs: 600_000 });
    if (result.status !== 0) throw new Error(`codec failed: ${headLines(result.stderr, 3)}`);
    const document = JSON.parse(result.stdout);
    document.artifacts.forEach((row, index) => sizes.set(batch[index][0], { raw: row.raw, gzip9: row.gzip9, brotli11: row.brotli11 }));
  }
  return sizes;
}

export async function runCases(options) {
  const started = new Date();
  const work = resolve(options.work);
  mkdirSync(work, { recursive: true });
  const compiler = pinBinary(options.compiler, join(work, "bin"));
  const codec = options.codec ? pinBinary(options.codec, join(work, "bin")) : null;
  const node = options.node;
  const cc = options.cc;
  const { env: compilerEnv, scrubbed } = compilerEnvironment();
  const runEnv = { PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, HOME: process.env.HOME ?? "/tmp", LANG: "C.UTF-8", NODE_OPTIONS: "" };
  const lanes = options.lanes;
  const ledger = loadLedger(options.ledger);
  const filters = (options.filter ?? "").split(",").map((item) => item.trim()).filter(Boolean)
    .map((item) => (item.includes("*") ? globMatcher(item) : (id) => id.includes(item)));
  const allCases = discoverCases(options.roots);
  const cases = allCases.filter((item) => !filters.length || filters.some((match) => match(item.id)));
  if (!cases.length) throw new Error("no case matches --filter");

  // The formation-only lane turns off every tactic the compiler's policy
  // lists; the list comes from the compiler, so the lane follows its registry.
  const probeSource = join(work, "policy-probe.lil");
  writeFileSync(probeSource, "print(1);\n");
  const printPolicy = async (config, target) => {
    const path = join(work, "policy-probe.toml");
    writeFileSync(path, config);
    const result = await run(compiler.path, [probeSource, "--config", path, "--target", target, "--print-policy"], { env: compilerEnv, cwd: work });
    try {
      return result.status === 0 ? JSON.parse(result.stdout) : { error: headLines(result.stderr, 3) };
    } catch {
      return { error: "unreadable --print-policy output" };
    }
  };
  let tactics = FALLBACK_TACTICS;
  let tacticSource = "fallback list (the compiler did not print a policy)";
  const baseline = await printPolicy(composeConfig(LANES.find((lane) => lane.mode === "production"), [], null), "js");
  if (Array.isArray(baseline?.policy?.tactics)) {
    tactics = baseline.policy.tactics.map((tactic) => tactic.id);
    tacticSource = "--print-policy";
  }
  const laneRecords = [];
  for (const lane of lanes) {
    const config = composeConfig(lane, tactics, null);
    const policy = await printPolicy(config, TARGETS[lane.target].flag);
    const record = { id: lane.id, mode: lane.mode, codec: lane.codec, target: lane.target, config };
    if (policy.error) record.policyError = policy.error;
    else {
      record.policyFingerprint = policy.fingerprint ?? null;
      record.enabledTactics = (policy.policy?.tactics ?? []).filter((tactic) => tactic.state?.enabled).map((tactic) => tactic.id);
      record.optionalAlternatives = policy.policy?.objective?.optional_alternatives ?? null;
      if (lane.mode === "formation-only" && (record.enabledTactics.length || record.optionalAlternatives)) {
        record.warning = "formation-only lane still enables optional work";
      }
    }
    laneRecords.push(record);
  }

  const tasks = [];
  for (const item of cases) for (const lane of lanes) tasks.push({ item, lane });
  const executions = new Map(); // identical programs run once per run
  let finished = 0;
  const progress = setInterval(() => process.stderr.write(`[cases] ${finished}/${tasks.length}\n`), 15_000);
  const results = await pool(tasks, options.jobs, async ({ item, lane }) => {
    const result = await runOne(item, lane);
    finished += 1;
    return result;
  });
  clearInterval(progress);

  async function runOne(item, lane) {
    const row = { case: item.id, lane: lane.id };
    const masked = maskFor(item, lane.target);
    if (masked.length) return { ...row, state: "masked", features: masked };
    const target = TARGETS[lane.target];
    const directory = join(work, "lanes", lane.id.replaceAll("/", "_"), dirname(item.id));
    mkdirSync(directory, { recursive: true });
    const base = join(directory, basename(item.id));
    const config = `${base}.toml`;
    try {
      writeFileSync(config, composeConfig(lane, tactics, item.toml ? readFileSync(item.toml, "utf8") : null));
    } catch (error) {
      return { ...row, state: "refused", detail: `case configuration: ${error.message}` };
    }
    const artifact = `${base}.${target.extension}`;
    rmSync(artifact, { force: true });
    const compile = await run(compiler.path, [item.source, "--target", target.flag, "--config", config, "--output", artifact], {
      env: compilerEnv, cwd: repository, timeoutMs: 300_000,
    });
    row.compileMs = compile.ms;
    writeFileSync(`${base}.compile.log`, compile.stdout + compile.stderr);
    if (compile.status !== 0 || compile.signal || compile.timedOut) {
      const crash = compile.signal || compile.timedOut || compile.status === 101 || compile.status === null;
      return { ...row, state: crash ? "compiler-crash" : "refused", detail: compile.timedOut ? "compiler timed out" : headLines(compile.stderr || compile.stdout, 3) };
    }
    if (!existsSync(artifact)) return { ...row, state: "refused", detail: "the compiler exited 0 without writing the artifact" };
    const bytes = readFileSync(artifact);
    row.artifact = { path: relative(work, artifact), bytes: bytes.length, sha256: sha256(bytes) };

    const expected = readFileSync(item.expected, "utf8");
    let execution;
    if (lane.target === "c") {
      const key = sha256(["c", cc, ...CC_FLAGS, row.artifact.sha256].join("\0"));
      if (!executions.has(key)) {
        executions.set(key, (async () => {
          const executable = `${base}.exe`;
          rmSync(executable, { force: true });
          const build = await run(cc, [...CC_FLAGS, artifact, "-lm", "-o", executable], { timeoutMs: 300_000 });
          if (build.status !== 0) return { cc: build };
          return { cc: build, run: await run(executable, [], { env: runEnv, cwd: directory, timeoutMs: options.timeoutMs }) };
        })());
      } else row.reused = true;
      execution = await executions.get(key);
      if (!execution.run) return { ...row, state: "cc-rejected", detail: keyLine(execution.cc.stderr) || headLines(execution.cc.stderr, 2) };
      execution = execution.run;
    } else {
      const prelude = item.host ? `${readFileSync(item.host, "utf8")}\n` : "";
      let program;
      let runner;
      if (item.probe) {
        runner = `${base}.run.mjs`;
        program = `${prelude}const m = await import(${JSON.stringify(pathToFileURL(artifact).href)});\n`
          + `const { default: probe } = await import(${JSON.stringify(pathToFileURL(item.probe).href)});\n`
          + "await probe(m);\n";
      } else {
        // One file: the prelude, then the compiled program, in one realm. The
        // script lane runs as CommonJS, as the harvest's verifiers did.
        runner = `${base}.run.${lane.target === "script" ? "cjs" : "mjs"}`;
        program = `${item.strict ? '"use strict";\n' : ""}${prelude}${bytes.toString("utf8")}\n`;
      }
      const key = sha256([lane.target, runner.slice(runner.lastIndexOf(".")), program, item.probe ? sha256File(item.probe) : ""].join("\0"));
      if (!executions.has(key)) {
        writeFileSync(runner, program);
        executions.set(key, run(node, [runner], { env: runEnv, cwd: directory, timeoutMs: options.timeoutMs }));
      } else row.reused = true;
      execution = await executions.get(key);
    }
    writeFileSync(`${base}.stdout`, execution.stdout);
    writeFileSync(`${base}.stderr`, execution.stderr);
    const difference = firstDifference(expected, execution.stdout);
    if (execution.status !== 0 || execution.signal || execution.timedOut) {
      const why = execution.timedOut ? `timed out after ${options.timeoutMs} ms` : execution.signal ? `killed by ${execution.signal}` : `exit ${execution.status}`;
      return { ...row, state: "crashed", detail: `${why}: ${keyLine(execution.stderr)}`, firstDifference: difference };
    }
    if (difference) return { ...row, state: "wrong-output", detail: `line ${difference.line}: expected ${JSON.stringify(difference.expected)}, got ${JSON.stringify(difference.actual)}`, firstDifference: difference };
    return { ...row, state: "pass" };
  }

  // Codec sizes of every JavaScript artifact, with the canonical encoders.
  const measured = results.filter((row) => row.artifact && TARGETS[row.lane.split("/")[2]].javascript);
  const sizes = await measureCodec(codec, measured.map((row) => ({ sha256: row.artifact.sha256, path: join(work, row.artifact.path) })));
  for (const row of measured) Object.assign(row.artifact, sizes.get(row.artifact.sha256) ?? {});

  for (const row of results) {
    if (!FAILURES.includes(row.state)) continue;
    const entry = ledger.entries.find((candidate) => candidate.matches(row.case, row.lane));
    if (entry) row.ledger = { index: entry.index, owner: entry.owner };
  }
  // An entry that covers no failure any more is stale and must go; one that
  // covers failures and passes could be narrowed (reported, not an error).
  const stale = [];
  const narrowable = [];
  for (const entry of ledger.entries) {
    const covered = results.filter((row) => entry.matches(row.case, row.lane) && row.state !== "masked");
    const passing = covered.filter((row) => row.state === "pass").map((row) => `${row.case} ${row.lane}`);
    if (!covered.length) continue;
    const record = { index: entry.index, case: entry.case, owner: entry.owner, passing };
    if (passing.length === covered.length) stale.push(record);
    else if (passing.length) narrowable.push(record);
  }
  const unknownEntries = options.filter ? [] : ledger.entries.flatMap((entry) => entry.patterns.filter((pattern) => !allCases.some((item) => globMatcher(pattern)(item.id))));

  const summary = {};
  for (const lane of lanes) {
    const rows = results.filter((row) => row.lane === lane.id);
    const count = (predicate) => rows.filter(predicate).length;
    summary[lane.id] = {
      cases: rows.length,
      pass: count((row) => row.state === "pass"),
      fail: count((row) => FAILURES.includes(row.state) && !row.ledger),
      ledgered: count((row) => FAILURES.includes(row.state) && row.ledger),
      masked: count((row) => row.state === "masked"),
    };
    const measuredRows = rows.filter((row) => row.artifact);
    summary[lane.id].artifactBytes = measuredRows.reduce((sum, row) => sum + row.artifact.bytes, 0);
    const metric = { brotli: "brotli11", gzip: "gzip9", raw: "raw" }[lane.codec];
    if (measuredRows.length && measuredRows.every((row) => row.artifact[metric] !== undefined)) {
      summary[lane.id][`${metric}Total`] = measuredRows.reduce((sum, row) => sum + row.artifact[metric], 0);
    }
  }
  const report = {
    schema: 1,
    kind: "lilscript-case-runner",
    started: started.toISOString(),
    completed: new Date().toISOString(),
    repository: { path: repository, ...gitIdentity(repository) },
    compiler,
    codec,
    node: { path: node, version: (await run(node, ["--version"])).stdout.trim() },
    cc: { path: cc, flags: CC_FLAGS, version: headLines((await run(cc, ["--version"])).stdout, 1) },
    scrubbedEnvironment: scrubbed,
    tactics: { source: tacticSource, ids: tactics },
    ledger: { path: ledger.path ? relative(repository, ledger.path) : null, sha256: ledger.sha256, entries: ledger.entries.length },
    masks: FEATURES.map(({ id, targets, why, lifts }) => ({ id, targets, why, lifts: lifts ?? null })),
    lanes: laneRecords,
    cases: cases.map(({ id, digest, features }) => ({ id, digest, features })),
    summary,
    unexpected: results.filter((row) => FAILURES.includes(row.state) && !row.ledger).length,
    staleLedgerEntries: stale,
    narrowableLedgerEntries: narrowable,
    unknownLedgerCases: unknownEntries,
    results,
  };
  if (options.compare) report.compare = compareReports(JSON.parse(readFileSync(options.compare, "utf8")), report, options.compare);
  return report;
}

// ---------------------------------------------------------------- comparison

// Byte changes against an earlier report, per case and lane. Evidence only:
// a change never fails the run.
export function compareReports(previous, current, label) {
  const index = new Map(previous.results.map((row) => [`${row.case}\t${row.lane}`, row]));
  const lanes = {};
  const states = [];
  const changes = [];
  for (const row of current.results) {
    const before = index.get(`${row.case}\t${row.lane}`);
    if (!before) continue;
    if (before.state !== row.state) states.push({ case: row.case, lane: row.lane, before: before.state, after: row.state });
    if (!before.artifact || !row.artifact) continue;
    const codec = row.lane.split("/")[1];
    const metric = { brotli: "brotli11", gzip: "gzip9", raw: "raw" }[codec];
    const lane = (lanes[row.lane] ??= { compared: 0, changed: 0, bytesBefore: 0, bytesAfter: 0, metric, metricBefore: 0, metricAfter: 0, metricComplete: true });
    lane.compared += 1;
    lane.bytesBefore += before.artifact.bytes;
    lane.bytesAfter += row.artifact.bytes;
    if (before.artifact[metric] !== undefined && row.artifact[metric] !== undefined) {
      lane.metricBefore += before.artifact[metric];
      lane.metricAfter += row.artifact[metric];
    } else lane.metricComplete = false;
    if (before.artifact.sha256 === row.artifact.sha256) continue;
    lane.changed += 1;
    changes.push({
      case: row.case, lane: row.lane,
      bytes: row.artifact.bytes - before.artifact.bytes,
      [metric]: before.artifact[metric] !== undefined && row.artifact[metric] !== undefined ? row.artifact[metric] - before.artifact[metric] : null,
    });
  }
  return {
    previous: label,
    previousCompiler: previous.compiler?.sha256 ?? null,
    sameCompiler: previous.compiler?.sha256 === current.compiler.sha256,
    lanes,
    stateChanges: states,
    changes,
  };
}

// ---------------------------------------------------------------- report

function lanesPhrase(laneIds, selected) {
  if (laneIds.length === selected.length) return "all lanes";
  const byTarget = Object.keys(TARGETS).filter((target) => {
    const all = selected.filter((lane) => lane.target === target).map((lane) => lane.id);
    return all.length && all.every((id) => laneIds.includes(id));
  });
  const rest = laneIds.filter((id) => !byTarget.includes(id.split("/")[2]));
  return [...byTarget.map((target) => `every ${target} lane`), ...rest].join(", ");
}

function printReport(report, selected, { verbose }) {
  const out = (line = "") => process.stdout.write(`${line}\n`);
  out(`compiler ${report.compiler.source} sha256 ${report.compiler.sha256.slice(0, 16)} (${report.compiler.version ?? "?"})`);
  out(`${report.cases.length} cases x ${selected.length} lanes; ledger ${report.ledger.path ?? "none"} (${report.ledger.entries} entries)`);
  if (report.scrubbedEnvironment.length) out(`ignored environment: ${report.scrubbedEnvironment.join(", ")}`);
  for (const lane of report.lanes) if (lane.warning || lane.policyError) out(`WARNING ${lane.id}: ${lane.warning ?? lane.policyError}`);
  out();
  const width = Math.max(...selected.map((lane) => lane.id.length));
  out(`${"lane".padEnd(width)}   pass  fail  ledgered  masked  artifact bytes  objective total`);
  for (const lane of selected) {
    const row = report.summary[lane.id];
    const metric = row.brotli11Total ?? row.gzip9Total ?? row.rawTotal;
    out(`${lane.id.padEnd(width)}  ${String(row.pass).padStart(5)} ${String(row.fail).padStart(5)} ${String(row.ledgered).padStart(9)} ${String(row.masked).padStart(7)} ${String(row.artifactBytes).padStart(15)} ${String(metric ?? "-").padStart(16)}`);
  }
  const failures = report.results.filter((row) => FAILURES.includes(row.state));
  const group = (rows) => {
    const groups = new Map();
    for (const row of rows) {
      const key = `${row.case}\0${row.state}\0${row.detail}`;
      if (!groups.has(key)) groups.set(key, { case: row.case, state: row.state, detail: row.detail, lanes: [] });
      groups.get(key).lanes.push(row.lane);
    }
    return [...groups.values()];
  };
  const unexpected = group(failures.filter((row) => !row.ledger));
  if (unexpected.length) {
    out();
    out(`UNEXPECTED FAILURES (${report.unexpected} case-lanes, ${new Set(unexpected.map((row) => row.case)).size} cases):`);
    for (const row of unexpected) out(`  ${row.case} [${lanesPhrase(row.lanes, selected)}] ${row.state}: ${row.detail.split("\n")[0]}`);
  }
  const ledgered = failures.filter((row) => row.ledger);
  if (ledgered.length) {
    out();
    out(`ledgered failures: ${ledgered.length} case-lanes in ${new Set(ledgered.map((row) => row.case)).size} cases`);
    if (verbose) for (const row of group(ledgered)) out(`  ${row.case} [${lanesPhrase(row.lanes, selected)}] ${row.state}: ${row.detail.split("\n")[0]}`);
  }
  if (report.staleLedgerEntries.length) {
    out();
    out("LEDGER ENTRIES THAT NOW PASS (remove them):");
    for (const entry of report.staleLedgerEntries) out(`  #${entry.index} ${[entry.case].flat().join(", ")} (${entry.owner}): passes on all ${entry.passing.length} covered lanes`);
  }
  if (verbose && report.narrowableLedgerEntries.length) {
    out();
    out("ledger entries that also cover passing lanes (could be narrowed):");
    for (const entry of report.narrowableLedgerEntries) out(`  #${entry.index} ${[entry.case].flat().join(", ")} (${entry.owner}): ${entry.passing.length} passing, e.g. ${entry.passing[0]}`);
  }
  if (report.unknownLedgerCases.length) out(`ledger entries naming no case: ${report.unknownLedgerCases.join(", ")}`);
  if (report.compare) {
    const compare = report.compare;
    out();
    out(`compared with ${compare.previous}${compare.sameCompiler ? " (same compiler digest)" : ""}:`);
    for (const [lane, row] of Object.entries(compare.lanes)) {
      const metric = row.metricComplete ? `, ${row.metric} ${row.metricBefore} -> ${row.metricAfter} (${row.metricAfter - row.metricBefore >= 0 ? "+" : ""}${row.metricAfter - row.metricBefore})` : "";
      out(`  ${lane.padEnd(width)} ${row.changed}/${row.compared} artifacts changed, bytes ${row.bytesBefore} -> ${row.bytesAfter}${metric}`);
    }
    for (const change of compare.stateChanges.slice(0, 40)) out(`  state ${change.case} ${change.lane}: ${change.before} -> ${change.after}`);
    const largest = [...compare.changes].sort((a, b) => Math.abs(b.bytes) - Math.abs(a.bytes)).slice(0, 15);
    for (const change of largest) out(`  bytes ${change.case} ${change.lane}: ${change.bytes >= 0 ? "+" : ""}${change.bytes}`);
  }
  out();
  out(report.unexpected ? `FAIL: ${report.unexpected} unexpected failing case-lanes` : "OK: no failure outside the ledger");
}

// ---------------------------------------------------------------- command line

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
      lanes: { type: "string", default: "all" },
      filter: { type: "string" },
      json: { type: "string" },
      compare: { type: "string" },
      jobs: { type: "string" },
      work: { type: "string", default: join(repository, "target/verify/cases") },
      cc: { type: "string", default: "/usr/bin/cc" },
      codec: { type: "string" },
      ledger: { type: "string", default: join(repository, "tests/cases/expected-failures.json") },
      node: { type: "string", default: process.execPath },
      timeout: { type: "string", default: "30" },
      verbose: { type: "boolean", default: false },
      help: { type: "boolean", default: false },
    },
  });
  if (values.help || !values.compiler) {
    process.stderr.write("usage: node scripts/cases.mjs --compiler <lilscript> [--lanes all|<mode>/<codec>/<target>,...] [--filter <id-substring|glob>,...] [--json out.json] [--compare previous.json] [--jobs N] [--work DIR] [--cc PATH] [--codec PATH|none] [--ledger FILE] [--node PATH] [--timeout SECONDS] [--verbose]\n");
    process.exit(values.help ? 0 : 2);
  }
  const selected = selectLanes(values.lanes);
  const codecPath = values.codec === "none" ? null : (values.codec ?? defaultCodec(values.compiler));
  const report = await runCases({
    compiler: values.compiler,
    lanes: selected,
    filter: values.filter,
    work: values.work,
    cc: values.cc,
    codec: codecPath,
    ledger: values.ledger === "none" ? null : resolve(values.ledger),
    node: resolve(values.node),
    jobs: Number(values.jobs ?? Math.max(1, availableParallelism() - 2)),
    timeoutMs: Number(values.timeout) * 1000,
    compare: values.compare ? resolve(values.compare) : null,
  });
  const destination = resolve(values.json ?? join(values.work, "report.json"));
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, `${JSON.stringify(report, null, 1)}\n`);
  printReport(report, selected, { verbose: values.verbose });
  process.stdout.write(`report: ${destination}\n`);
  process.exitCode = report.unexpected ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(new URL(import.meta.url).pathname)) {
  main().catch((error) => {
    process.stderr.write(`cases: ${error.stack ?? error.message}\n`);
    process.exit(2);
  });
}
