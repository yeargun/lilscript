import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const evidence = JSON.parse(
  readFileSync(resolve(root, "compatibility/runtime-test.json"), "utf8"),
);
const files = [["packages/runtime-test/__tests__/testRuntime.spec.ts", 5]];
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-test-tests-"));
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

  for (const script of ["build-runtime-core.mjs", "build-runtime-test.mjs"]) {
    const build = spawnSync(process.execPath, [resolve(root, "scripts", script)], {
      cwd: root,
      encoding: "utf8",
      env: process.env,
    });
    assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);
  }

  const result = spawnSync(process.execPath, [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config", resolve(root, "tests/runtime-test-upstream.config.mjs"),
    ...files.map(([path]) => path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${reportPath}`,
  ], {
    cwd: upstream,
    encoding: "utf8",
    env: { ...process.env, VUELIL_RUNTIME_TEST_AUDIT: auditPath },
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
  }
  assert.equal(result.status, 0, "unchanged runtime-test suite failed");

  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const audit = JSON.parse(readFileSync(auditPath, "utf8"));
  const assertionsByPath = new Map(report.testResults.map(file => [
    relative(upstream, file.name).replaceAll("\\", "/"),
    file.assertionResults,
  ]));
  assert.deepEqual([...assertionsByPath.keys()].sort(), files.map(([path]) => path).sort());
  for (const [path, count] of files) {
    assert.equal(assertionsByPath.get(path).length, count, `${path} test count changed`);
    assert.ok(assertionsByPath.get(path).every(test => test.status === "passed"));
  }
  assert.deepEqual(
    evidence.upstreamTests.unchangedFiles,
    files.map(([path, tests]) => ({ path, tests })),
  );
  assert.equal(evidence.upstreamTests.passed, 5);
  assert.equal(report.numTotalTests, 5);
  assert.equal(report.numPassedTests, 5);
  assert.equal(report.numPendingTests, 0);
  assert.equal(report.numFailedTests, 0);
  assert.equal(audit.candidateLoaded, true);
  assert.equal(audit.coreLoaded, true);
  assert.deepEqual(audit.blocked, []);
  console.log("VueLil runtime-test passed 5/5 tests in its unchanged upstream suite.");
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
