import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const upstream = resolve(root, "upstream/vue");
const inventory = JSON.parse(readFileSync(resolve(root, "compatibility/inventory.json"), "utf8"));
const evidence = JSON.parse(readFileSync(resolve(root, "compatibility/vue.json"), "utf8"));
const inventoryFiles = inventory.packages.find(entry => entry.name === "vue").testFiles;
const files = evidence.upstreamTests.unchangedFiles;
const browserFiles = [
  "packages/vue/__tests__/e2e/Transition.spec.ts",
  "packages/vue/__tests__/e2e/TransitionGroup.spec.ts",
];
const nodeFiles = inventoryFiles.filter(path => !browserFiles.includes(path));
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-vue-tests-"));
const globalPath = resolve(upstream, "packages/vue/dist/vue.global.js");
const previousGlobal = existsSync(globalPath) ? readFileSync(globalPath) : null;
const { default: puppeteer } = await import(pathToFileURL(resolve(
  upstream,
  "node_modules/puppeteer/lib/puppeteer/puppeteer.js",
)).href);
const chromiumExecutable = await puppeteer.executablePath();

function runBuild(script) {
  const result = spawnSync(process.execPath, [resolve(root, `scripts/${script}`)], {
    cwd: root,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 64 * 1024 * 1024,
  });
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
}

function runTests(name, files, browser) {
  const reportPath = resolve(temporary, `${name}.json`);
  const auditPath = resolve(temporary, `${name}-audit.json`);
  const result = spawnSync(process.execPath, [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config", resolve(root, "tests/vue-upstream.config.mjs"),
    ...files,
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${reportPath}`,
  ], {
    cwd: upstream,
    encoding: "utf8",
    env: {
      ...process.env,
      CI: "1",
      VUELIL_VUE_AUDIT: auditPath,
      VUELIL_VUE_BROWSER: browser ? "1" : "0",
      VUELIL_CHROMIUM_EXECUTABLE: chromiumExecutable,
    },
    maxBuffer: 128 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    if (existsSync(reportPath)) process.stderr.write(readFileSync(reportPath, "utf8"));
  }
  assert.equal(result.status, 0, `${name} unchanged Vue tranche failed`);
  return {
    report: JSON.parse(readFileSync(reportPath, "utf8")),
    audit: JSON.parse(readFileSync(auditPath, "utf8")),
  };
}

function assertionsByPath(report) {
  return new Map(report.testResults.map(file => [
    relative(upstream, file.name).replaceAll("\\", "/"),
    file.assertionResults,
  ]));
}

try {
  assert.equal(inventory.upstream.version, "3.5.42", "inventory Vue version changed");
  assert.equal(inventory.upstream.revision, "d63616ca17de965ed32dcb449a4c5cd9982f15d2");
  assert.equal(inventoryFiles.length, 18, "pinned Vue test-file inventory changed");
  assert.deepEqual(files.map(({ path }) => path), inventoryFiles);
  assert.deepEqual([...nodeFiles, ...browserFiles].sort(), inventoryFiles.toSorted());

  const revision = spawnSync("git", ["rev-parse", "HEAD"], { cwd: upstream, encoding: "utf8" });
  assert.equal(revision.status, 0, revision.stderr);
  assert.equal(revision.stdout.trim(), inventory.upstream.revision, "upstream revision changed");
  const initialStatus = spawnSync("git", ["status", "--porcelain", "--untracked-files=no"], {
    cwd: upstream,
    encoding: "utf8",
  });
  assert.equal(initialStatus.status, 0, initialStatus.stderr);
  assert.equal(initialStatus.stdout, "", "upstream Vue checkout has tracked changes");

  for (const script of [
    "build-runtime-core.mjs",
    "build-runtime-dom.mjs",
    "build-compiler-core.mjs",
    "build-compiler-dom.mjs",
    "build-vue.mjs",
  ]) runBuild(script);

  const stamp = Date.now();
  const [vue, runtimeOnly, runtimeDom, compilerDom, reactivity, shared] = await Promise.all([
    import(`../packages/vuelil/vue.js?test=${stamp}`),
    import(`../packages/vuelil/vue.runtime.js?test=${stamp}`),
    import("../packages/vuelil/runtime-dom.js"),
    import("../packages/vuelil/compiler-dom.js"),
    import("../packages/vuelil/reactivity.js"),
    import("../packages/vuelil/shared.js"),
  ]);
  const expectedExports = [...new Set([...Object.keys(runtimeDom), "compile"])].sort();
  assert.deepEqual(Object.keys(vue).sort(), expectedExports);
  assert.deepEqual(Object.keys(runtimeOnly).sort(), expectedExports);
  for (const name of Object.keys(runtimeDom)) {
    assert.equal(vue[name], runtimeDom[name], `${name} full-build singleton identity`);
    assert.equal(runtimeOnly[name], runtimeDom[name], `${name} runtime-only singleton identity`);
  }
  for (const name of Object.keys(reactivity).filter(
    name => name in vue && name !== "computed" && name !== "watch",
  )) {
    assert.equal(vue[name], reactivity[name], `${name} reactivity singleton identity`);
  }
  for (const name of ["camelize", "capitalize", "normalizeClass", "normalizeProps", "normalizeStyle", "toDisplayString", "toHandlerKey"]) {
    assert.equal(vue[name], shared[name], `${name} shared singleton identity`);
  }
  assert.notEqual(vue.compile, compilerDom.compile, "Vue compile must be the registered runtime compiler wrapper");
  assert.equal(vue.compile("<p>ok</p>")._rc, true, "full build did not register runtime compilation");

  mkdirSync(resolve(upstream, "packages/vue/dist"), { recursive: true });
  writeFileSync(globalPath, readFileSync(resolve(root, "packages/vuelil/vue.global.test.js")));

  const node = runTests("node", nodeFiles, false);
  const browser = runTests("browser", browserFiles, true);
  const combined = new Map([
    ...assertionsByPath(node.report),
    ...assertionsByPath(browser.report),
  ]);
  assert.deepEqual([...combined.keys()].sort(), inventoryFiles.toSorted());
  for (const [path, assertions] of combined) {
    const expected = files.find(file => file.path === path);
    assert.ok(assertions.length > 0, `${path} contains no tests`);
    assert.ok(
      assertions.every(test => test.status === "passed" || test.status === "skipped"),
      `${path} contains a failed test`,
    );
    assert.equal(assertions.filter(test => test.status === "passed").length, expected.passed);
    assert.equal(assertions.filter(test => test.status === "skipped").length, expected.skipped);
  }
  assert.equal(node.report.numFailedTests + browser.report.numFailedTests, 0);
  assert.equal(node.report.numPassedTests + browser.report.numPassedTests, 144);
  assert.equal(node.report.numPendingTests + browser.report.numPendingTests, 0);
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
      files: 18,
      filesPassed: 18,
      tests: 144,
      passed: 144,
      skipped: 0,
      failed: 0,
      status: "passed",
    },
  );
  assert.deepEqual(node.audit.blockedUpstreamImplementations, []);
  assert.deepEqual(browser.audit.blockedUpstreamImplementations, []);
  for (const packageName of [
    "vue",
    "@vue/compiler-core",
    "@vue/compiler-dom",
    "@vue/reactivity",
    "@vue/runtime-core",
    "@vue/runtime-dom",
    "@vue/shared",
  ]) {
    assert.ok(node.audit.loadedCandidates.includes(packageName), `${packageName} candidate was not loaded`);
  }
  assert.ok(node.audit.redirects.some(entry => entry.startsWith("vue:")));
  assert.equal(
    readFileSync(globalPath).equals(readFileSync(resolve(root, "packages/vuelil/vue.global.test.js"))),
    true,
    "e2e tests did not use the VueLil browser global",
  );

  const passed = [...combined.values()].flat().filter(test => test.status === "passed").length;
  const skipped = [...combined.values()].flat().filter(test => test.status === "skipped").length;
  console.log(`VueLil vue passed ${passed} tests with ${skipped} skipped in all 18 unchanged upstream files.`);
  console.log(JSON.stringify(Object.fromEntries(
    [...combined].map(([path, assertions]) => [path, {
      passed: assertions.filter(test => test.status === "passed").length,
      skipped: assertions.filter(test => test.status === "skipped").length,
    }]),
  ), null, 2));
} finally {
  if (previousGlobal) writeFileSync(globalPath, previousGlobal);
  else if (existsSync(globalPath)) unlinkSync(globalPath);
  rmSync(temporary, { force: true, recursive: true });
}
