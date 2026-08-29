import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const files = [
  ["packages/runtime-core/__tests__/scheduler.spec.ts", 39],
  ["packages/runtime-core/__tests__/h.spec.ts", 6],
  ["packages/runtime-core/__tests__/helpers/renderList.spec.ts", 13],
  ["packages/runtime-core/__tests__/helpers/toHandlers.spec.ts", 2],
  ["packages/runtime-core/__tests__/rendererElement.spec.ts", 6],
  ["packages/runtime-core/__tests__/rendererChildren.spec.ts", 38],
  ["packages/runtime-core/__tests__/rendererFragment.spec.ts", 13],
  ["packages/runtime-core/__tests__/rendererComponent.spec.ts", 16],
  ["packages/runtime-core/__tests__/apiCreateApp.spec.ts", 22],
  ["packages/runtime-core/__tests__/apiInject.spec.ts", 19],
  ["packages/runtime-core/__tests__/apiLifecycle.spec.ts", 14],
  ["packages/runtime-core/__tests__/vnode.spec.ts", 56],
];
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-core-tests-"));
const reportPath = resolve(temporary, "vitest.json");
const auditPath = resolve(temporary, "audit.json");

try {
  const upstreamPackage = JSON.parse(readFileSync(resolve(upstream, "package.json"), "utf8"));
  assert.equal(upstreamPackage.version, "3.5.42", "upstream Vue version changed");
  const revision = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: upstream,
    encoding: "utf8",
  });
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

  const build = spawnSync(process.execPath, [resolve(root, "scripts/build-runtime-core.mjs")], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);

  const result = spawnSync(process.execPath, [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config", resolve(root, "tests/runtime-core-upstream.config.mjs"),
    ...files.map(([path]) => path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${reportPath}`,
  ], {
    cwd: upstream,
    encoding: "utf8",
    env: { ...process.env, VUELIL_RUNTIME_CORE_AUDIT: auditPath },
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
  }
  assert.equal(result.status, 0, "unchanged runtime-core tranche failed");

  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const audit = JSON.parse(readFileSync(auditPath, "utf8"));
  const tests = report.testResults.flatMap(file => file.assertionResults);
  assert.equal(report.testResults.length, files.length);
  assert.equal(tests.length, files.reduce((sum, [, count]) => sum + count, 0));
  assert.ok(tests.every(test => test.status === "passed"));
  assert.equal(audit.candidateLoaded, true);
  assert.deepEqual(audit.blocked, []);
  console.log(`VueLil runtime-core passed ${tests.length}/${tests.length} tests in ${files.length} unchanged upstream files.`);
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
