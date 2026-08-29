import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const files = [
  ["packages/runtime-dom/__tests__/patchAttrs.spec.ts", 9],
  ["packages/runtime-dom/__tests__/patchClass.spec.ts", 3],
  ["packages/runtime-dom/__tests__/patchStyle.spec.ts", 19],
  ["packages/runtime-dom/__tests__/patchEvents.spec.ts", 17],
  ["packages/runtime-dom/__tests__/directives/vOn.spec.ts", 9],
];
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-dom-tests-"));
const reportPath = resolve(temporary, "vitest.json");
const auditPath = resolve(temporary, "audit.json");

try {
  const upstreamPackage = JSON.parse(readFileSync(resolve(upstream, "package.json"), "utf8"));
  assert.equal(upstreamPackage.version, "3.5.42", "upstream Vue version changed");
  const revision = spawnSync("git", ["rev-parse", "HEAD"], { cwd: upstream, encoding: "utf8" });
  assert.equal(revision.status, 0, revision.stderr);
  assert.equal(
    revision.stdout.trim(),
    "d63616ca17de965ed32dcb449a4c5cd9982f15d2",
    "upstream Vue revision changed",
  );
  const status = spawnSync("git", ["status", "--porcelain", "--untracked-files=no"], {
    cwd: upstream,
    encoding: "utf8",
  });
  assert.equal(status.status, 0, status.stderr);
  assert.equal(status.stdout, "", "upstream Vue checkout has tracked changes");

  const build = spawnSync(process.execPath, [resolve(root, "scripts/build-runtime-dom.mjs")], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);

  const result = spawnSync(process.execPath, [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config", resolve(root, "tests/runtime-dom-upstream.config.mjs"),
    ...files.map(([path]) => path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${reportPath}`,
  ], {
    cwd: upstream,
    encoding: "utf8",
    env: { ...process.env, VUELIL_RUNTIME_DOM_AUDIT: auditPath },
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
  }
  assert.equal(result.status, 0, "unchanged runtime-dom tranche failed");

  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const audit = JSON.parse(readFileSync(auditPath, "utf8"));
  const tests = report.testResults.flatMap(file => file.assertionResults);
  assert.equal(report.testResults.length, files.length);
  assert.equal(tests.length, files.reduce((sum, [, count]) => sum + count, 0));
  assert.ok(tests.every(test => test.status === "passed"));
  assert.equal(audit.candidateLoaded, true);
  assert.deepEqual(audit.blocked, []);
  console.log(`VueLil runtime-dom passed ${tests.length}/${tests.length} tests in ${files.length} unchanged upstream files.`);
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
