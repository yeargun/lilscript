import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const files = [
  ["packages/runtime-core/__tests__/apiAsyncComponent.spec.ts", 25],
  ["packages/runtime-core/__tests__/apiCreateApp.spec.ts", 22],
  ["packages/runtime-core/__tests__/apiExpose.spec.ts", 11],
  ["packages/runtime-core/__tests__/apiInject.spec.ts", 19],
  ["packages/runtime-core/__tests__/apiLifecycle.spec.ts", 14],
  ["packages/runtime-core/__tests__/apiOptions.spec.ts", 48],
  ["packages/runtime-core/__tests__/apiSetupContext.spec.ts", 7],
  ["packages/runtime-core/__tests__/apiSetupHelpers.spec.ts", 20],
  ["packages/runtime-core/__tests__/apiWatch.spec.ts", 85],
  ["packages/runtime-core/__tests__/component.spec.ts", 9],
  ["packages/runtime-core/__tests__/componentEmits.spec.ts", 28],
  ["packages/runtime-core/__tests__/componentProps.spec.ts", 28],
  ["packages/runtime-core/__tests__/componentPublicInstance.spec.ts", 20],
  ["packages/runtime-core/__tests__/componentSlots.spec.ts", 17],
  ["packages/runtime-core/__tests__/components/BaseTransition.spec.ts", 39],
  ["packages/runtime-core/__tests__/components/KeepAlive.spec.ts", 32],
  ["packages/runtime-core/__tests__/components/Suspense.spec.ts", 59],
  ["packages/runtime-core/__tests__/components/Teleport.spec.ts", 56],
  ["packages/runtime-core/__tests__/directives.spec.ts", 7],
  ["packages/runtime-core/__tests__/errorHandling.spec.ts", 21],
  ["packages/runtime-core/__tests__/h.spec.ts", 6],
  ["packages/runtime-core/__tests__/helpers/createSlots.spec.ts", 6],
  ["packages/runtime-core/__tests__/helpers/renderList.spec.ts", 13],
  ["packages/runtime-core/__tests__/helpers/renderSlot.spec.ts", 14],
  ["packages/runtime-core/__tests__/helpers/resolveAssets.spec.ts", 7],
  ["packages/runtime-core/__tests__/helpers/toHandlers.spec.ts", 2],
  ["packages/runtime-core/__tests__/helpers/useId.spec.ts", 8],
  ["packages/runtime-core/__tests__/helpers/useModel.spec.ts", 15],
  ["packages/runtime-core/__tests__/helpers/useTemplateRef.spec.ts", 15],
  ["packages/runtime-core/__tests__/helpers/withMemo.spec.ts", 9],
  ["packages/runtime-core/__tests__/hmr.spec.ts", 25],
  ["packages/runtime-core/__tests__/hydration.spec.ts", 125],
  ["packages/runtime-core/__tests__/misc.spec.ts", 1],
  ["packages/runtime-core/__tests__/rendererAttrsFallthrough.spec.ts", 22],
  ["packages/runtime-core/__tests__/rendererChildren.spec.ts", 38],
  ["packages/runtime-core/__tests__/rendererComponent.spec.ts", 16],
  ["packages/runtime-core/__tests__/rendererElement.spec.ts", 6],
  ["packages/runtime-core/__tests__/rendererFragment.spec.ts", 13],
  ["packages/runtime-core/__tests__/rendererOptimizedMode.spec.ts", 35],
  ["packages/runtime-core/__tests__/rendererTemplateRef.spec.ts", 24],
  ["packages/runtime-core/__tests__/scheduler.spec.ts", 39],
  ["packages/runtime-core/__tests__/scopeId.spec.ts", 12],
  ["packages/runtime-core/__tests__/vnode.spec.ts", 56],
  ["packages/runtime-core/__tests__/vnodeHooks.spec.ts", 2],
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
  const assertionsByPath = new Map(
    report.testResults.map(file => [
      relative(upstream, file.name).replaceAll("\\", "/"),
      file.assertionResults,
    ]),
  );
  const tests = report.testResults.flatMap(file => file.assertionResults);
  assert.equal(report.testResults.length, files.length);
  assert.deepEqual(
    [...assertionsByPath.keys()].sort(),
    files.map(([path]) => path).sort(),
  );
  for (const [path, count] of files) {
    assert.equal(assertionsByPath.get(path).length, count, `${path} test count changed`);
  }
  assert.equal(tests.length, files.reduce((sum, [, count]) => sum + count, 0));
  assert.ok(tests.every(test => test.status === "passed"));
  assert.equal(audit.candidateLoaded, true);
  assert.deepEqual(audit.blocked, []);
  console.log(`VueLil runtime-core passed ${tests.length}/${tests.length} tests in ${files.length} unchanged upstream files.`);
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
