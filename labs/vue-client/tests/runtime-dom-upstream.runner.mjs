import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const evidence = JSON.parse(
  readFileSync(resolve(root, "compatibility/runtime-dom.json"), "utf8"),
);
const files = [
  ["packages/runtime-dom/__tests__/createApp.spec.ts", 2, 0],
  ["packages/runtime-dom/__tests__/customElement.spec.ts", 82, 1],
  ["packages/runtime-dom/__tests__/customizedBuiltIn.spec.ts", 1, 0],
  ["packages/runtime-dom/__tests__/directives/vCloak.spec.ts", 1, 0],
  ["packages/runtime-dom/__tests__/directives/vModel.spec.ts", 40, 0],
  ["packages/runtime-dom/__tests__/directives/vOn.spec.ts", 9, 0],
  ["packages/runtime-dom/__tests__/directives/vShow.spec.ts", 9, 0],
  ["packages/runtime-dom/__tests__/helpers/useCssModule.spec.ts", 5, 0],
  ["packages/runtime-dom/__tests__/helpers/useCssVars.spec.ts", 17, 0],
  ["packages/runtime-dom/__tests__/nodeOps.spec.ts", 8, 0],
  ["packages/runtime-dom/__tests__/patchAttrs.spec.ts", 9, 0],
  ["packages/runtime-dom/__tests__/patchClass.spec.ts", 3, 0],
  ["packages/runtime-dom/__tests__/patchEvents.spec.ts", 17, 0],
  ["packages/runtime-dom/__tests__/patchProps.spec.ts", 26, 0],
  ["packages/runtime-dom/__tests__/patchStyle.spec.ts", 19, 0],
  ["packages/runtime-dom/__tests__/rendererStaticNode.spec.ts", 5, 0],
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

  const [candidate, oracle] = await Promise.all([
    import(`../packages/vuelil/runtime-dom.js?upstream=${Date.now()}`),
    import("@vue/runtime-dom"),
  ]);
  const oracleExports = Object.keys(oracle)
    .filter(name => !["__esModule", "default", "module.exports"].includes(name))
    .sort();
  assert.equal(oracleExports.length, 170, "upstream runtime-dom export count changed");
  assert.deepEqual(Object.keys(candidate).sort(), oracleExports);
  assert.equal(evidence.runtimeExportCount, 170);
  assert.deepEqual(evidence.runtimeExports, oracleExports);
  assert.deepEqual(
    evidence.upstreamTests.unchangedFiles,
    files.map(([path, passed, skipped]) => ({ path, passed, skipped })),
  );
  assert.deepEqual(
    {
      files: evidence.upstreamTests.files,
      passed: evidence.upstreamTests.passed,
      skipped: evidence.upstreamTests.skipped,
      failed: evidence.upstreamTests.failed,
    },
    { files: 16, passed: 253, skipped: 1, failed: 0 },
  );

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
  const assertionsByPath = new Map(report.testResults.map(file => [
    relative(upstream, file.name).replaceAll("\\", "/"),
    file.assertionResults,
  ]));
  assert.deepEqual([...assertionsByPath.keys()].sort(), files.map(([path]) => path).sort());
  for (const [path, passed, skipped] of files) {
    const assertions = assertionsByPath.get(path);
    assert.equal(assertions.length, passed + skipped, `${path} test count changed`);
    assert.equal(
      assertions.filter(test => test.status === "passed").length,
      passed,
      `${path} passed count changed`,
    );
    assert.equal(
      assertions.filter(test => test.status === "skipped").length,
      skipped,
      `${path} skipped count changed`,
    );
    assert.ok(assertions.every(test => test.status === "passed" || test.status === "skipped"));
  }
  assert.equal(report.numTotalTests, 254);
  assert.equal(report.numPassedTests, 253);
  assert.equal(report.numPendingTests, 1);
  assert.equal(report.numFailedTests, 0);
  assert.equal(audit.candidateLoaded, true);
  assert.deepEqual(audit.blocked, []);
  console.log("VueLil runtime-dom passed 253 tests with 1 skipped in all 16 unchanged upstream files.");
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
