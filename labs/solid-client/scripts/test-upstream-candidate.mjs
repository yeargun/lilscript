import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { root } from "./project.mjs";

const upstream = resolve(root, "upstream", "solid");
const packageRoot = resolve(upstream, "packages", "solid");
const config = resolve(root, "tooling", "solidlil-upstream-vite.config.mjs");
const rawReport = resolve(
  tmpdir(),
  `solidlil-upstream-candidate-${process.pid}.json`,
);
const result = spawnSync(
  "corepack",
  [
    "pnpm",
    "exec",
    "vitest",
    "run",
    "--config",
    config,
    "--maxWorkers=1",
    "--minWorkers=1",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${rawReport}`,
  ],
  { cwd: packageRoot, encoding: "utf8", env: process.env },
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
  readFileSync(resolve(packageRoot, "package.json"), "utf8"),
);
const summary = {
  candidate: "SolidLil",
  suite: `${packageDefinition.name}@${packageDefinition.version}`,
  revision,
  testFiles: report.testResults.length,
  tests: report.numTotalTests,
  passed: report.numPassedTests,
  failed: report.numFailedTests,
  success: report.success,
  sourcePolicy:
    "Pinned upstream test files are executed unchanged; public solid-js, solid-js/web, and solid-js/store entries resolve to SolidLil.",
};

assert.equal(summary.testFiles, 26, "the candidate test-file count changed");
assert.equal(summary.tests, 469, "the candidate runtime-test count changed");
assert.equal(summary.failed, 0, "SolidLil has upstream runtime failures");
assert.equal(summary.success, true, "the SolidLil candidate suite failed");

const artifacts = resolve(root, "artifacts");
mkdirSync(artifacts, { recursive: true });
writeFileSync(
  resolve(artifacts, "solidlil-upstream-candidate.json"),
  `${JSON.stringify(summary, null, 2)}\n`,
);
writeFileSync(
  resolve(artifacts, "solidlil-upstream-candidate.md"),
  `# SolidLil upstream-candidate test gate

| Candidate | Pinned suite | Revision | Files | Tests | Passed | Failed |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| ${summary.candidate} | ${summary.suite} | \`${summary.revision.slice(0, 12)}\` | ${summary.testFiles} | ${summary.tests} | ${summary.passed} | ${summary.failed} |

${summary.sourcePolicy}
`,
);

console.log(
  `SolidLil passed ${summary.passed}/${summary.tests} unchanged upstream tests in ${summary.testFiles} files.`,
);
