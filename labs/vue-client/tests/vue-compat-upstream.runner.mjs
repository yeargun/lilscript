import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const evidence = JSON.parse(
  readFileSync(resolve(root, "compatibility/vue-compat.json"), "utf8"),
);
const files = [
  ["packages/vue-compat/__tests__/compiler.spec.ts", 9, 0],
  ["packages/vue-compat/__tests__/componentAsync.spec.ts", 3, 0],
  ["packages/vue-compat/__tests__/componentFunctional.spec.ts", 2, 0],
  ["packages/vue-compat/__tests__/componentVModel.spec.ts", 5, 0],
  ["packages/vue-compat/__tests__/filters.spec.ts", 17, 0],
  ["packages/vue-compat/__tests__/global.spec.ts", 30, 0],
  ["packages/vue-compat/__tests__/globalConfig.spec.ts", 4, 0],
  ["packages/vue-compat/__tests__/instance.spec.ts", 21, 0],
  ["packages/vue-compat/__tests__/misc.spec.ts", 10, 0],
  ["packages/vue-compat/__tests__/options.spec.ts", 5, 0],
  ["packages/vue-compat/__tests__/renderFn.spec.ts", 11, 0],
];
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-vue-compat-tests-"));
const reportPath = resolve(temporary, "vitest.json");
const auditPath = resolve(temporary, "audit.json");

function runBuild(script) {
  const result = spawnSync(process.execPath, [resolve(root, `scripts/${script}`)], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 64 * 1024 * 1024,
  });
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
}

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

  for (const script of [
    "build-runtime-core.mjs",
    "build-runtime-dom.mjs",
    "build-compiler-core.mjs",
    "build-compiler-dom.mjs",
    "build-vue-compat.mjs",
  ]) runBuild(script);

  const [standard, runtimeOnly, runtimeDom] = await Promise.all([
    import(`../packages/vuelil/vue-compat.js?test=${Date.now()}`),
    import(`../packages/vuelil/vue-compat.runtime.js?test=${Date.now()}`),
    import("../packages/vuelil/runtime-dom.compat.js"),
  ]);
  const expectedExports = [...new Set([
    ...evidence.esmEntries.additionalExports,
    ...Object.keys(runtimeDom),
  ])].sort();
  assert.equal(Object.keys(runtimeDom).length, evidence.esmEntries.runtimeDomExports);
  assert.equal(expectedExports.length, evidence.esmEntries.totalExports);
  assert.deepEqual(Object.keys(standard).sort(), expectedExports);
  assert.deepEqual(Object.keys(runtimeOnly).sort(), expectedExports);
  for (const name of Object.keys(runtimeDom)) {
    assert.equal(standard[name], runtimeDom[name], `${name} standard singleton identity`);
    assert.equal(runtimeOnly[name], runtimeDom[name], `${name} runtime-only singleton identity`);
  }
  assert.equal(standard.configureCompat, standard.default.configureCompat);
  assert.equal(runtimeOnly.configureCompat, runtimeOnly.default.configureCompat);
  const render = standard.default.compile("<div>compat</div>", { whitespace: "preserve" });
  assert.equal(typeof render, "function");
  assert.equal(render._rc, true);
  const warnings = [];
  const consoleWarn = console.warn;
  console.warn = (...args) => warnings.push(args.join(" "));
  try {
    assert.equal(runtimeOnly.default.compile("<div/>"), undefined);
  } finally {
    console.warn = consoleWarn;
  }
  assert.ok(warnings.some(message => message.includes(evidence.esmEntries.runtimeOnlyCompileWarning)));

  assert.deepEqual(
    evidence.upstreamTests.unchangedFiles,
    files.map(([path, passed, skipped]) => ({ path, passed, skipped })),
  );
  const result = spawnSync(process.execPath, [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config", resolve(root, "tests/vue-compat-upstream.config.mjs"),
    ...files.map(([path]) => path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${reportPath}`,
  ], {
    cwd: upstream,
    encoding: "utf8",
    env: { ...process.env, CI: "1", VUELIL_VUE_COMPAT_AUDIT: auditPath },
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
  }
  assert.equal(result.status, 0, "unchanged vue-compat tranche failed");

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
    assert.equal(assertions.filter(test => test.status === "passed").length, passed);
    assert.equal(assertions.filter(test => test.status === "skipped").length, skipped);
  }
  assert.equal(report.numTotalTests, 117);
  assert.equal(report.numPassedTests, 117);
  assert.equal(report.numPendingTests, 0);
  assert.equal(report.numFailedTests, 0);
  assert.deepEqual(audit.blockedUpstreamImplementations, []);
  assert.deepEqual(audit.loadedCandidates, [
    "@vue/compat",
    "@vue/compiler-core",
    "@vue/compiler-dom",
    "@vue/reactivity",
    "@vue/runtime-core",
    "@vue/runtime-dom",
    "@vue/shared",
  ]);
  assert.ok(audit.redirects.some(entry => entry.startsWith("@vue/compat:")));
  assert.ok(audit.redirects.some(entry => entry.startsWith("@vue/runtime-core:")));
  assert.ok(audit.redirects.some(entry => entry.startsWith("@vue/compiler-core:")));
  assert.deepEqual(
    {
      files: evidence.upstreamTests.files,
      filesPassed: evidence.upstreamTests.filesPassed,
      tests: evidence.upstreamTests.tests,
      passed: evidence.upstreamTests.passed,
      skipped: evidence.upstreamTests.skipped,
      failed: evidence.upstreamTests.failed,
      status: evidence.upstreamTests.status,
    },
    {
      files: 11,
      filesPassed: 11,
      tests: 117,
      passed: 117,
      skipped: 0,
      failed: 0,
      status: "passed",
    },
  );
  console.log("VueLil @vue/compat passed 117/117 tests in all 11 unchanged upstream files.");
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
