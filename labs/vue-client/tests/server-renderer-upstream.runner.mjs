import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const files = [
  ["packages/server-renderer/__tests__/render.spec.ts", 171],
  ["packages/server-renderer/__tests__/ssrAttrFallthrough.spec.ts", 5],
  ["packages/server-renderer/__tests__/ssrCompilerOptions.spec.ts", 9],
  ["packages/server-renderer/__tests__/ssrComputed.spec.ts", 2],
  ["packages/server-renderer/__tests__/ssrDirectives.spec.ts", 25],
  ["packages/server-renderer/__tests__/ssrDynamicComponent.spec.ts", 5],
  ["packages/server-renderer/__tests__/ssrInterpolate.spec.ts", 1],
  ["packages/server-renderer/__tests__/ssrRender.spec.ts", 2],
  ["packages/server-renderer/__tests__/ssrRenderAttrs.spec.ts", 24],
  ["packages/server-renderer/__tests__/ssrRenderList.spec.ts", 13],
  ["packages/server-renderer/__tests__/ssrScopeId.spec.ts", 7],
  ["packages/server-renderer/__tests__/ssrSlot.spec.ts", 13],
  ["packages/server-renderer/__tests__/ssrSuspense.spec.ts", 7],
  ["packages/server-renderer/__tests__/ssrTeleport.spec.ts", 8],
  ["packages/server-renderer/__tests__/ssrVModelHelpers.spec.ts", 2],
  ["packages/server-renderer/__tests__/ssrWatch.spec.ts", 12],
  ["packages/server-renderer/__tests__/webStream.spec.ts", 2],
];
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-server-renderer-tests-"));
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

  const buildScripts = [
    "build-runtime-core.mjs",
    "build-runtime-dom.mjs",
    "build-runtime-test.mjs",
    "build-compiler-ssr.mjs",
    "build-server-renderer.mjs",
  ];
  for (const script of buildScripts) {
    const build = spawnSync(process.execPath, [resolve(root, "scripts", script)], {
      cwd: root,
      encoding: "utf8",
      env: process.env,
      maxBuffer: 64 * 1024 * 1024,
    });
    assert.equal(build.status, 0, `${build.stdout}${build.stderr}`);
  }

  const result = spawnSync(process.execPath, [
    "--expose-gc",
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config", resolve(root, "tests/server-renderer-upstream.config.mjs"),
    ...files.map(([path]) => path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${reportPath}`,
  ], {
    cwd: upstream,
    encoding: "utf8",
    env: { ...process.env, VUELIL_SERVER_RENDERER_AUDIT: auditPath },
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
  }
  assert.equal(result.status, 0, "unchanged server-renderer suites failed");

  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const audit = JSON.parse(readFileSync(auditPath, "utf8"));
  const assertionsByPath = new Map(report.testResults.map(file => [
    relative(upstream, file.name).replaceAll("\\", "/"),
    file.assertionResults,
  ]));
  assert.deepEqual([...assertionsByPath.keys()].sort(), files.map(([path]) => path).sort());
  for (const [path, count] of files) {
    assert.equal(assertionsByPath.get(path).length, count, `${path} test count changed`);
    assert.ok(assertionsByPath.get(path).every(test => test.status === "passed"), path);
  }
  assert.equal(report.numTotalTests, 308);
  assert.equal(report.numPassedTests, 308);
  assert.ok(audit.loaded.includes("serverRenderer"));
  assert.ok(audit.loaded.includes("runtimeCore"));
  assert.ok(audit.loaded.includes("runtimeDom"));
  assert.equal(audit.runtimeTestAlias, resolve(root, "packages/vuelil/runtime-test.js"));
  assert.deepEqual(audit.blocked, []);
  console.log("VueLil server-renderer passed 308/308 tests in all 17 unchanged upstream files.");
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
