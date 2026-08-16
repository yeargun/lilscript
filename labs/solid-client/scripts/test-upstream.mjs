import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { root } from "./project.mjs";

const upstream = resolve(root, "upstream", "solid");
const rawReport = resolve(
  tmpdir(),
  `solid-upstream-vitest-${process.pid}.json`,
);
const result = spawnSync(
  "corepack",
  [
    "pnpm",
    "--filter",
    "solid-js",
    "exec",
    "vitest",
    "run",
    "--maxWorkers=1",
    "--minWorkers=1",
    "--reporter=json",
    `--outputFile=${rawReport}`,
  ],
  { cwd: upstream, encoding: "utf8", env: process.env },
);

if (result.status !== 0) {
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}

const report = JSON.parse(readFileSync(rawReport, "utf8"));
const revision = spawnSync("git", ["rev-parse", "HEAD"], {
  cwd: upstream,
  encoding: "utf8",
}).stdout.trim();
const packageDefinition = JSON.parse(
  readFileSync(resolve(upstream, "packages", "solid", "package.json"), "utf8"),
);
const summary = {
  package: `${packageDefinition.name}@${packageDefinition.version}`,
  revision,
  testFiles: report.testResults.length,
  tests: report.numTotalTests,
  passed: report.numPassedTests,
  failed: report.numFailedTests,
  success: report.success,
};

assert.equal(
  summary.testFiles,
  26,
  "the pinned upstream test-file count changed",
);
assert.equal(
  summary.tests,
  469,
  "the pinned upstream runtime-test count changed",
);
assert.equal(
  summary.failed,
  0,
  "the official Solid baseline has test failures",
);
assert.equal(
  summary.success,
  true,
  "the official Solid baseline did not succeed",
);

writeFileSync(
  resolve(root, "artifacts", "upstream-solid-tests.json"),
  `${JSON.stringify(summary, null, 2)}\n`,
);
writeFileSync(
  resolve(root, "artifacts", "upstream-solid-tests.md"),
  `# Official SolidJS test baseline

| Package | Revision | Files | Tests | Passed | Failed |
| --- | --- | ---: | ---: | ---: | ---: |
| ${summary.package} | \`${summary.revision.slice(0, 12)}\` | ${summary.testFiles} | ${summary.tests} | ${summary.passed} | ${summary.failed} |

This executes the unchanged upstream Vitest configuration. It is the reference
suite; it does not count as LilScript compatibility.
`,
);

console.log(
  `Official ${summary.package} baseline passed: ${summary.passed}/${summary.tests} tests in ${summary.testFiles} files.`,
);
