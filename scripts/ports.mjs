#!/usr/bin/env node
// The port runner (plan task M2.6): maintained ports' own suites against one
// pinned compiler, diffed against the expected-failure ledger.
//
//   node scripts/ports.mjs --compiler <lilscript> --ports a,b|all
//        [--objective shipped|brotli|gzip|raw] [--json out.json] [--work DIR]
//        [--ports-root ~] [--ledger tests/ports/expected-failures.json]
//        [--timeout SECONDS] [--jobs N] [--codec <lilscript-codec>|none] [--keep]
//
// For each port the runner:
//   1. copies the port (without .git, dist, _site, .tmp, test-output; every
//      node_modules is linked, not copied) into a scratch parent that links the
//      sibling ports and this repository (`../lilscript`), as ports expect;
//   2. applies finer/port-migrations/<port>.patch when present;
//   3. with --objective other than `shipped`, rewrites `cost_model` in every
//      copied lilscript*.toml;
//   4. builds with LILSCRIPT_COMPILER and MOTIONLIL_LILSCRIPT_BIN pointing at a
//      logging wrapper around the pinned compiler (LILSCRIPT_* knobs from the
//      caller's environment are dropped), measures dist/ with the canonical
//      codec, then runs `npm test` (or `npm run check:site` for a port with no
//      test script; a port without a package is its own differential build);
//   5. records the failing-test SET and diffs it against the ledger: a failure
//      the ledger does not list is a regression (exit 1); a listed failure
//      that passes is reported for removal.
// The port is never built in place: its checkout under ~ is only read.
// See docs/testing.md.
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { parseArgs } from "node:util";
import { gitIdentity, headLines, pinBinary, pool, repository, run, sha256File } from "./lib/verify-util.mjs";

export const OBJECTIVES = ["shipped", "brotli", "gzip", "raw"];
const SKIP_AT_ROOT = new Set([".git", "dist", "_site", ".tmp", "test-output", "node_modules"]);

// ---------------------------------------------------------------- ports

export function maintainedPorts() {
  const manifest = JSON.parse(readFileSync(join(repository, "benchmarks/libraries/maintained-workloads.json"), "utf8"));
  return manifest.libraries.map((row) => row.id);
}

// Copies a port. Every node_modules directory (at any depth) becomes a link to
// the original, so installs are shared and never copied.
function copyPort(source, destination) {
  mkdirSync(destination, { recursive: true });
  const walk = (from, to, depth) => {
    for (const entry of readdirSync(from, { withFileTypes: true })) {
      const origin = join(from, entry.name);
      const target = join(to, entry.name);
      if (entry.name === "node_modules") {
        symlinkSync(origin, target);
        continue;
      }
      if (entry.name === ".git" || (depth === 0 && SKIP_AT_ROOT.has(entry.name))) continue;
      if (entry.isSymbolicLink()) {
        symlinkSync(readlinkSync(origin), target);
      } else if (entry.isDirectory()) {
        mkdirSync(target);
        walk(origin, target, depth + 1);
      } else {
        cpSync(origin, target, { verbatimSymlinks: true });
      }
    }
  };
  walk(source, destination, 0);
}

function configFiles(directory) {
  const found = [];
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      if (entry.name === "node_modules" || entry.name === ".git") continue;
      const path = join(current, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (/^lilscript.*\.toml$/.test(entry.name)) found.push(path);
    }
  };
  walk(directory);
  return found.sort();
}

// Sets `[javascript] cost_model` (the key, wherever it is; else inserted
// after the `[javascript]` header; else a new table).
export function rewriteObjective(text, objective) {
  const value = `cost_model = ${JSON.stringify(objective)}`;
  if (/^\s*cost_model\s*=.*$/m.test(text)) return text.replace(/^(\s*)cost_model\s*=.*$/gm, (_, indent) => `${indent}${value}`);
  if (/^\[javascript\]\s*$/m.test(text)) return text.replace(/^\[javascript\]\s*$/m, `[javascript]\n${value}`);
  return `${text.replace(/\n*$/, "\n")}\n[javascript]\n${value}\n`;
}

function distArtifacts(workspace) {
  const dist = join(workspace, "dist");
  if (!existsSync(dist)) return [];
  const found = [];
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (/\.(?:js|mjs|cjs)$/.test(entry.name)) found.push(path);
    }
  };
  walk(dist);
  return found.sort();
}

// ---------------------------------------------------------------- test output

// Totals from every runner the ports use: node:test TAP (`# tests N`) and spec
// (`ℹ tests N`), mocha (`N passing`), differential scripts (`N/M fixtures
// passed`), jest (`Tests: … N passed, M total`), vitest (`Tests  N passed
// (M)`) and upstream harnesses (`pass: N  fail: M`).
export function testTotals(text) {
  let tests = 0;
  let pass = 0;
  let fail = 0;
  let seen = false;
  const add = (total, passed, failed) => { tests += total; pass += passed; fail += failed; seen = true; };
  const sum = (pattern) => [...text.matchAll(pattern)].reduce((total, match) => total + Number(match[1]), 0);
  if (/^(?:#|ℹ) tests \d+$/m.test(text)) add(sum(/^(?:#|ℹ) tests (\d+)$/gm), sum(/^(?:#|ℹ) pass (\d+)$/gm), sum(/^(?:#|ℹ) fail (\d+)$/gm));
  if (/^\s*\d+ passing\b/m.test(text)) {
    const passed = sum(/^\s*(\d+) passing\b/gm);
    const failed = sum(/^\s*(\d+) failing\b/gm);
    add(passed + failed, passed, failed);
  }
  for (const match of text.matchAll(/(\d+)\/(\d+) fixtures passed/g)) add(Number(match[2]), Number(match[1]), Number(match[2]) - Number(match[1]));
  const jest = text.match(/^Tests:\s+(?:(\d+) failed, )?(?:(\d+) skipped, )?(?:(\d+) todo, )?(\d+) passed, (\d+) total/m);
  if (jest) add(Number(jest[5]), Number(jest[4]), Number(jest[1] ?? 0));
  const vitest = text.match(/Tests\s+(?:(\d+) failed \| )?(\d+) passed(?: \| \d+ \w+)* \((\d+)\)/);
  if (vitest) add(Number(vitest[3]), Number(vitest[2]), Number(vitest[1] ?? 0));
  for (const match of text.matchAll(/\bpass:?\s+(\d+)\s+fail:?\s+(\d+)/g)) add(Number(match[1]) + Number(match[2]), Number(match[1]), Number(match[2]));
  return seen ? { tests, pass, fail } : { tests: null, pass: null, fail: null };
}

// The names of failing tests, as a set. node:test TAP (`not ok N - name`,
// TODO/SKIP directives excluded) and spec (`✖ name (1.2ms)`), jest (`● a › b`,
// `✕ name`), vitest (`× name`, ` FAIL  file > name`) and mocha (`1) name`).
export function failingTests(text) {
  const names = new Set();
  const add = (name) => {
    const clean = name.replace(/\s+\([\d.]+\s*m?s\)\s*$/, "").trim();
    if (clean && clean !== "failing tests:" && clean !== "Console") names.add(clean);
  };
  const mocha = /^\s*\d+ failing\b/m.test(text);
  for (const line of text.split("\n")) {
    let match;
    if ((match = /^\s*not ok \d+\s*-?\s*(.*?)\s*$/.exec(line))) {
      if (!/#\s*(?:TODO|SKIP)\b/i.test(match[1])) add(match[1].replace(/\s+#.*$/, ""));
    } else if ((match = /^\s*✖ (.*)$/.exec(line))) add(match[1]);
    else if ((match = /^\s+● (.*)$/.exec(line))) add(match[1]);
    else if ((match = /^\s*[✕×] (.*)$/.exec(line))) add(match[1]);
    else if ((match = /^\s*FAIL\s+(\S.*? > .*)$/.exec(line))) add(match[1]);
    else if (mocha && (match = /^\s{2,}\d+\) (.*)$/.exec(line))) add(match[1].replace(/:$/, ""));
  }
  return [...names].sort();
}

// ---------------------------------------------------------------- ledger

export function loadLedger(path) {
  if (!path || !existsSync(path)) return { path: path ?? null, sha256: null, entries: [] };
  const document = JSON.parse(readFileSync(path, "utf8"));
  const entries = (document.entries ?? []).map((entry, index) => {
    for (const field of ["port", "reason", "owner"]) {
      if (typeof entry[field] !== "string" || !entry[field].trim()) throw new Error(`${path}: entry ${index} needs a non-empty "${field}"`);
    }
    if (!Array.isArray(entry.tests) || !entry.tests.length) throw new Error(`${path}: entry ${index} needs a non-empty "tests" list`);
    return { ...entry, index };
  });
  return { path, sha256: sha256File(path), entries };
}

// ---------------------------------------------------------------- one port

async function runPort(port, context) {
  const { compiler, codec, work, portsRoot, objective, ledger, timeoutMs, keep, log } = context;
  const source = join(portsRoot, port);
  const row = { port, source: { path: source } };
  if (!existsSync(source)) return { ...row, state: "missing", failing: [], regressions: [], nowPassing: [] };
  Object.assign(row.source, gitIdentity(source));

  const parent = join(work, "runs", port);
  rmSync(parent, { recursive: true, force: true });
  mkdirSync(parent, { recursive: true });
  // Ports import sibling ports' sources (`../unifiedlil/src`) and find this
  // repository beside them (`../lilscript`).
  for (const sibling of readdirSync(portsRoot)) {
    if (sibling === port || !(sibling.endsWith("lil") || sibling.startsWith("lil-"))) continue;
    const path = join(portsRoot, sibling);
    try { if (!lstatSync(path).isDirectory()) continue; } catch { continue; }
    symlinkSync(path, join(parent, sibling));
  }
  symlinkSync(repository, join(parent, "lilscript"));
  const workspace = join(parent, port);
  copyPort(source, workspace);

  const failing = [];
  const logs = join(work, "logs");
  mkdirSync(logs, { recursive: true });
  const finish = (extra = {}) => {
    const result = { ...row, ...extra };
    result.failing = [...new Set([...failing, ...(result.test?.failing ?? [])])].sort();
    const listed = ledger.entries.filter((entry) => entry.port === port);
    const expected = new Set(listed.flatMap((entry) => entry.tests));
    result.ledgered = result.failing.filter((name) => expected.has(name));
    result.regressions = result.failing.filter((name) => !expected.has(name));
    // A listed failure can be seen passing only when the suite ran.
    result.nowPassing = result.test ? [...expected].filter((name) => !result.failing.includes(name)) : [];
    result.ledgerEntries = listed.map((entry) => ({ index: entry.index, owner: entry.owner }));
    result.state = result.regressions.length ? "regressed" : result.failing.length ? "ledgered" : "green";
    if (!keep) rmSync(parent, { recursive: true, force: true });
    return result;
  };

  const patch = join(repository, "finer", "port-migrations", `${port}.patch`);
  if (existsSync(patch)) {
    const applied = await run("patch", ["-p1", "--forward", "--batch", "-d", workspace, "-i", patch], { timeoutMs: 120_000 });
    row.patch = { path: relative(repository, patch), sha256: sha256File(patch), status: applied.status };
    if (applied.status !== 0) {
      row.patch.error = headLines(applied.stdout + applied.stderr, 12);
      failing.push("(patch failed)");
      return finish();
    }
  }

  if (objective !== "shipped") {
    row.objectiveRewrites = [];
    for (const file of configFiles(workspace)) {
      const text = readFileSync(file, "utf8");
      const before = /^\s*cost_model\s*=\s*(.*)$/m.exec(text)?.[1] ?? null;
      writeFileSync(file, rewriteObjective(text, objective));
      row.objectiveRewrites.push({ file: relative(workspace, file), before });
    }
  }

  // Every compiler call goes through a wrapper that logs it, so a run that
  // compiled nothing (a stale dist, a hard-coded binary) cannot pass silently.
  const invocationLog = join(parent, "compiler-invocations.log");
  const wrapper = join(parent, "lilscript-wrapper");
  writeFileSync(wrapper, `#!/bin/sh\nprintf '%s\\n' "$*" >> ${JSON.stringify(invocationLog)}\nexec ${JSON.stringify(compiler.path)} "$@"\n`, { mode: 0o755 });
  const invocations = () => (existsSync(invocationLog) ? readFileSync(invocationLog, "utf8").split("\n").filter(Boolean).length : 0);
  const env = {};
  for (const [key, value] of Object.entries(process.env)) if (!key.startsWith("LILSCRIPT_") && key !== "MOTIONLIL_LILSCRIPT_BIN") env[key] = value;
  Object.assign(env, {
    LILSCRIPT_COMPILER: wrapper,
    MOTIONLIL_LILSCRIPT_BIN: wrapper,
    LILSCRIPT_ROOT: repository,
    PATH: `${dirname(process.execPath)}:${process.env.PATH ?? "/usr/bin:/bin"}`,
    npm_config_update_notifier: "false",
  });
  // Ports measure with `$LILSCRIPT_CODEC`, else `$LILSCRIPT_ROOT/target/release`.
  if (codec) env.LILSCRIPT_CODEC = codec.path;
  const step = async (name, command, args) => {
    log(`${port}: ${name}: ${[command, ...args].join(" ")}`);
    const result = await run(command, args, { cwd: workspace, env, timeoutMs });
    writeFileSync(join(logs, `${port}.${name}.log`), `${result.stdout}\n${result.stderr}`);
    return result;
  };

  const packagePath = join(workspace, "package.json");
  if (!existsSync(packagePath)) {
    // A port without a package (the probe) is a differential build: it
    // compiles, runs and compares its own output, so that build is its test.
    const probe = await step("differential", process.execPath, ["scripts/build.mjs", "--compile"]);
    const passed = probe.status === 0;
    row.compilerInvocations = invocations();
    if (!row.compilerInvocations) failing.push("(compiler not invoked)");
    return finish({ test: { command: "node scripts/build.mjs --compile", status: probe.status, ms: probe.ms, tests: 1, pass: passed ? 1 : 0, fail: passed ? 0 : 1, failing: passed ? [] : ["(differential build failed)"], tail: headLines((probe.stdout + probe.stderr).split("\n").slice(-12).join("\n"), 12) } });
  }
  const scripts = JSON.parse(readFileSync(packagePath, "utf8")).scripts ?? {};
  if (scripts.build) {
    // Some builds reuse existing output unless asked to compile (and katexlil
    // keeps an mtime cache unless forced).
    const script = join(workspace, "scripts", "build.mjs");
    const text = existsSync(script) ? readFileSync(script, "utf8") : "";
    let command = ["npm", ["run", "build"]];
    if (text.includes("--compile") && !scripts.build.includes("--compile")) {
      command = [process.execPath, ["scripts/build.mjs", "--compile", ...(text.includes('"--force"') || text.includes("'--force'") ? ["--force"] : [])]];
    }
    const build = await step("build", ...command);
    row.build = {
      command: [basename(command[0]), ...command[1]].join(" "), status: build.status, ms: build.ms, timedOut: build.timedOut,
      compilerInvocations: invocations(),
      errors: (build.stdout + build.stderr).split("\n").filter((line) => /error|refus|unsupported/i.test(line) && !/^warning/i.test(line)).slice(0, 10),
    };
    if (build.status !== 0) failing.push(build.timedOut ? "(build timed out)" : "(build failed)");
    const artifacts = distArtifacts(workspace);
    row.artifacts = artifacts.map((path) => ({ path: relative(workspace, path), bytes: readFileSync(path).length, sha256: sha256File(path) }));
    if (codec && artifacts.length) {
      const measured = await run(codec.path, ["--json", ...artifacts], { timeoutMs: 600_000 });
      if (measured.status === 0) {
        JSON.parse(measured.stdout).artifacts.forEach((size, index) => Object.assign(row.artifacts[index], { raw: size.raw, gzip9: size.gzip9, brotli11: size.brotli11 }));
      } else row.measurementError = headLines(measured.stderr, 3);
    }
  }

  const suite = scripts.test ? ["test", ["test"]] : scripts["check:site"] ? ["check:site", ["run", "check:site"]] : null;
  if (!suite) {
    failing.push("(no npm test script)");
    row.compilerInvocations = invocations();
    return finish();
  }
  const test = await step("test", "npm", suite[1]);
  const text = `${test.stdout}\n${test.stderr}`;
  const names = failingTests(text);
  if (test.timedOut) names.push("(suite timed out)");
  else if (test.status !== 0 && !names.length) names.push("(suite failed without named failures)");
  row.compilerInvocations = invocations();
  if (!row.compilerInvocations) failing.push("(compiler not invoked)");
  return finish({
    test: {
      command: `npm ${suite[1].join(" ")}`, status: test.status, signal: test.signal, ms: test.ms,
      ...testTotals(text), failing: names, tail: text.trim().split("\n").slice(-15).join("\n"),
    },
  });
}

// ---------------------------------------------------------------- command line

function printReport(report) {
  const out = (line = "") => process.stdout.write(`${line}\n`);
  out(`compiler ${report.compiler.source} sha256 ${report.compiler.sha256.slice(0, 16)}; objective ${report.objective}; ledger ${report.ledger.path} (${report.ledger.entries} entries)`);
  out();
  const width = Math.max(4, ...report.ports.map((row) => row.port.length));
  out(`${"port".padEnd(width)}  state      build        suite              failing  ledgered  new  now-passing  compiles`);
  for (const row of report.ports) {
    const build = row.build ? `${row.build.status === 0 ? "ok" : "FAIL"} ${Math.round(row.build.ms / 1000)}s` : row.state === "missing" ? "-" : "none";
    const suite = row.test ? `${row.test.pass ?? "?"}/${row.test.tests ?? "?"} exit ${row.test.status}` : "-";
    out(`${row.port.padEnd(width)}  ${row.state.padEnd(9)}  ${build.padEnd(11)}  ${suite.padEnd(17)}  ${String(row.failing.length).padStart(7)}  ${String(row.ledgered?.length ?? 0).padStart(8)}  ${String(row.regressions.length).padStart(3)}  ${String(row.nowPassing.length).padStart(11)}  ${String(row.compilerInvocations ?? "-").padStart(8)}`);
  }
  for (const row of report.ports) {
    if (row.patch?.error) out(`\n${row.port}: patch failed:\n${row.patch.error}`);
    if (row.regressions.length) {
      out(`\n${row.port}: NEW FAILURES (not in the ledger):`);
      for (const name of row.regressions) out(`  ${name}`);
      if (row.build?.errors?.length) out(`  build errors:\n    ${row.build.errors.join("\n    ")}`);
    }
    if (row.nowPassing.length) {
      out(`\n${row.port}: ledgered failures that now pass (remove them from the ledger):`);
      for (const name of row.nowPassing) out(`  ${name}`);
    }
  }
  const missing = report.ports.filter((row) => row.state === "missing").map((row) => row.port);
  if (missing.length) out(`\nmissing under ${report.portsRoot}: ${missing.join(", ")}`);
  out();
  out(report.regressions ? `FAIL: ${report.regressions} failures outside the ledger` : "OK: no failure outside the ledger");
}

async function main() {
  const { values } = parseArgs({
    options: {
      compiler: { type: "string" },
      ports: { type: "string" },
      objective: { type: "string", default: "shipped" },
      json: { type: "string" },
      work: { type: "string", default: join(repository, "target/verify/ports") },
      "ports-root": { type: "string", default: homedir() },
      ledger: { type: "string", default: join(repository, "tests/ports/expected-failures.json") },
      timeout: { type: "string", default: "2700" },
      jobs: { type: "string", default: "1" },
      codec: { type: "string" },
      keep: { type: "boolean", default: false },
      help: { type: "boolean", default: false },
    },
  });
  if (values.help || !values.compiler || !values.ports) {
    process.stderr.write("usage: node scripts/ports.mjs --compiler <lilscript> --ports a,b|all [--objective shipped|brotli|gzip|raw] [--json out.json] [--work DIR] [--ports-root DIR] [--ledger FILE] [--timeout SECONDS] [--jobs N] [--codec PATH|none] [--keep]\n");
    process.exit(values.help ? 0 : 2);
  }
  if (!OBJECTIVES.includes(values.objective)) throw new Error(`--objective must be one of ${OBJECTIVES.join(", ")}`);
  const known = maintainedPorts();
  const portsRoot = resolve(values["ports-root"]);
  // `all` is the maintained libraries that exist under the ports root.
  const ports = values.ports === "all" ? known.filter((port) => existsSync(join(portsRoot, port))) : values.ports.split(",").map((port) => port.trim()).filter(Boolean);
  for (const port of ports) if (!/^[a-z][a-z0-9-]*$/.test(port)) throw new Error(`invalid port name: ${port}`);
  const work = resolve(values.work);
  mkdirSync(work, { recursive: true });
  const compiler = pinBinary(values.compiler, join(work, "bin"));
  const codecPath = values.codec === "none" ? null
    : values.codec ?? [join(dirname(resolve(values.compiler)), "lilscript-codec"), join(repository, "target/release/lilscript-codec")].find(existsSync) ?? null;
  const codec = codecPath ? pinBinary(codecPath, join(work, "bin")) : null;
  const ledger = loadLedger(values.ledger === "none" ? null : resolve(values.ledger));
  const started = new Date();
  const log = (line) => process.stderr.write(`[ports] ${line}\n`);
  log(`compiler ${compiler.sha256.slice(0, 16)}, ${ports.length} ports, objective ${values.objective}`);
  const rows = await pool(ports, Number(values.jobs), (port) => runPort(port, {
    compiler, codec, work, portsRoot, objective: values.objective, ledger, timeoutMs: Number(values.timeout) * 1000, keep: values.keep, log,
  }).catch((error) => {
    // One port's runner fault is that port's failure, not the run's end.
    const failing = [`(runner error: ${error.message.split("\n")[0]})`];
    return { port, state: "regressed", error: error.stack, failing, ledgered: [], regressions: failing, nowPassing: [] };
  }).then((row) => {
    log(`${port}: ${row.state}${row.regressions.length ? ` (${row.regressions.length} new)` : ""}`);
    return row;
  }));
  const pinnedAfter = sha256File(compiler.path);
  const report = {
    schema: 1,
    kind: "lilscript-port-runner",
    started: started.toISOString(),
    completed: new Date().toISOString(),
    repository: { path: repository, ...gitIdentity(repository) },
    compiler: { ...compiler, unchangedDuringRun: pinnedAfter === compiler.sha256 },
    codec,
    node: process.version,
    objective: values.objective,
    portsRoot,
    ledger: { path: ledger.path ? relative(repository, ledger.path) : null, sha256: ledger.sha256, entries: ledger.entries.length },
    unknownPorts: ports.filter((port) => !known.includes(port)),
    ports: rows,
    regressions: rows.reduce((sum, row) => sum + row.regressions.length, 0),
  };
  const destination = resolve(values.json ?? join(work, "report.json"));
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, `${JSON.stringify(report, null, 1)}\n`);
  printReport(report);
  process.stdout.write(`report: ${destination}\n`);
  if (!report.compiler.unchangedDuringRun) process.stdout.write("FAIL: the pinned compiler changed during the run\n");
  process.exitCode = report.regressions || !report.compiler.unchangedDuringRun ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(new URL(import.meta.url).pathname)) {
  main().catch((error) => {
    process.stderr.write(`ports: ${error.stack ?? error.message}\n`);
    process.exit(2);
  });
}

