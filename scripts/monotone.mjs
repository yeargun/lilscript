#!/usr/bin/env node
// The effort schedule's monotonicity check (plan M3.5, architecture §9.6).
//
//   node scripts/monotone.mjs --compiler <lilscript> [--codec <lilscript-codec>]
//        [--levels 0,1,5,10,11,12,13,14,15,16] [--lanes production/*/module]
//        [--filter <spec>] [--jobs N] [--work DIR] [--json out.json]
//
// Every case is compiled and run (scripts/cases.mjs) at each level of the
// list, in the production JavaScript lanes, and measured under its lane's
// codec. A level keeps the walk of every level below it and judges more, so
// a case-lane whose artifact grows from one listed level to the next fails
// the check; so does one that passes at a level and fails at the next. The
// default list is the schedule's tier boundaries: one level per tier.
//
// See docs/testing.md.
import { mkdirSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { runCases, selectLanes } from "./cases.mjs";
import { repository } from "./lib/verify-util.mjs";

const METRIC = { brotli: "brotli11", gzip: "gzip9", raw: "raw" };

export async function checkMonotone(options) {
  const levels = [];
  for (const level of options.levels) {
    const report = await runCases({ ...options, work: join(options.work, `level-${level}`), level, compare: null });
    levels.push({ level, report });
    process.stderr.write(`[monotone] level ${level}: ${report.results.length} case-lanes, ${report.unexpected} unexpected failures\n`);
  }
  const key = (row) => `${row.case}\0${row.lane}`;
  const violations = [];
  const totals = [];
  for (const { level, report } of levels) {
    const total = {};
    for (const row of report.results) {
      if (row.state !== "pass" || !row.artifact) continue;
      const metric = METRIC[row.lane.split("/")[1]];
      total[row.lane] = (total[row.lane] ?? 0) + (row.artifact[metric] ?? 0);
    }
    totals.push({ level, lanes: total });
  }
  for (let index = 1; index < levels.length; index += 1) {
    const lower = new Map(levels[index - 1].report.results.map((row) => [key(row), row]));
    for (const row of levels[index].report.results) {
      const below = lower.get(key(row));
      if (!below || below.state === "masked") continue;
      const at = { case: row.case, lane: row.lane, from: levels[index - 1].level, to: levels[index].level };
      if (below.state === "pass" && row.state !== "pass") {
        violations.push({ ...at, kind: "state", before: below.state, after: row.state, detail: row.detail ?? null });
        continue;
      }
      if (below.state !== "pass" || row.state !== "pass") continue;
      const metric = METRIC[row.lane.split("/")[1]];
      const before = below.artifact?.[metric];
      const after = row.artifact?.[metric];
      if (before === undefined || after === undefined) continue;
      if (after > before) violations.push({ ...at, kind: "size", metric, before, after });
    }
  }
  return { schema: 1, kind: "lilscript-monotone", levels: options.levels, totals, violations };
}

async function main() {
  const { values } = parseArgs({
    options: {
      compiler: { type: "string" },
      codec: { type: "string" },
      levels: { type: "string", default: "0,1,5,10,11,12,13,14,15,16" },
      lanes: { type: "string", default: "production/*/module" },
      filter: { type: "string" },
      jobs: { type: "string" },
      work: { type: "string", default: join(repository, "target/verify/monotone") },
      json: { type: "string" },
      ledger: { type: "string", default: join(repository, "tests/cases/expected-failures.json") },
      node: { type: "string", default: process.execPath },
      cc: { type: "string", default: "/usr/bin/cc" },
      timeout: { type: "string", default: "30" },
      help: { type: "boolean", default: false },
    },
  });
  if (values.help || !values.compiler) {
    process.stderr.write("usage: node scripts/monotone.mjs --compiler <lilscript> [--codec PATH] [--levels 0,1,5,...] [--lanes spec] [--filter spec] [--jobs N] [--work DIR] [--json out.json]\n");
    process.exit(values.help ? 0 : 2);
  }
  const lanes = selectLanes(values.lanes).filter((lane) => lane.mode === "production" && lane.target !== "c");
  if (!lanes.length) throw new Error("--lanes selects no production JavaScript lane");
  const levels = values.levels.split(",").map((level) => Number(level.trim()));
  if (levels.some((level) => !Number.isInteger(level) || level < 0)) throw new Error("--levels takes effort levels");
  const result = await checkMonotone({
    compiler: values.compiler,
    codec: values.codec ?? join(dirname(resolve(values.compiler)), "lilscript-codec"),
    lanes,
    levels,
    filter: values.filter,
    work: resolve(values.work),
    cc: values.cc,
    ledger: values.ledger === "none" ? null : resolve(values.ledger),
    node: resolve(values.node),
    jobs: Number(values.jobs ?? Math.max(1, availableParallelism() - 2)),
    timeoutMs: Number(values.timeout) * 1000,
  });
  const destination = resolve(values.json ?? join(values.work, "monotone.json"));
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, `${JSON.stringify(result, null, 1)}\n`);
  for (const { level, lanes: total } of result.totals) {
    process.stdout.write(`level ${String(level).padStart(2)}  ${Object.entries(total).map(([lane, size]) => `${lane} ${size}`).join("  ")}\n`);
  }
  for (const violation of result.violations) {
    process.stdout.write(violation.kind === "size"
      ? `GROWS ${violation.case} ${violation.lane}: level ${violation.from} ${violation.before} -> level ${violation.to} ${violation.after} ${violation.metric}\n`
      : `STATE ${violation.case} ${violation.lane}: level ${violation.from} ${violation.before} -> level ${violation.to} ${violation.after}\n`);
  }
  process.stdout.write(result.violations.length ? `FAIL: ${result.violations.length} violations\n` : "OK: monotone at every listed level\n");
  process.stdout.write(`report: ${destination}\n`);
  process.exitCode = result.violations.length ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(new URL(import.meta.url).pathname)) {
  main().catch((error) => {
    process.stderr.write(`monotone: ${error.stack ?? error.message}\n`);
    process.exit(2);
  });
}
