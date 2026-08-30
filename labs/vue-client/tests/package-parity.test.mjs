import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const require = createRequire(import.meta.url);
const inventory = readJson("compatibility/inventory.json");
const scope = readJson("compatibility/scope.json");
const report = readJson("artifacts/compatibility-report.json");

function readJson(path) {
  return JSON.parse(readFileSync(resolve(root, path), "utf8"));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function assertArtifact(entry) {
  const bytes = readFileSync(resolve(root, entry.path));
  assert.equal(bytes.byteLength, entry.bytes, `${entry.path} byte count`);
  assert.equal(sha256(bytes), entry.sha256, `${entry.path} hash`);
}

test("aggregate compatibility evidence is exhaustive and current", () => {
  assert.equal(report.schemaVersion, 1);
  assert.equal(report.upstream.revision, inventory.upstream.revision);
  assert.equal(report.upstream.tree, inventory.upstream.tree);
  assert.equal(
    report.inventory.sha256,
    sha256(readFileSync(resolve(root, "compatibility/inventory.json"))),
  );
  assert.equal(report.noUpstreamImplementationImports, true);
  assert.deepEqual(report.packageResolution, {
    import: "passed",
    require: "passed",
  });
  assert.deepEqual(report.totals, {
    packages: 12,
    packageEntrypoints: 28,
    publicExports: 2023,
    declarationExports: 2015,
    runtimeExports: 1301,
    runtimeDescriptors: 1301,
    formatRows: 38,
    formatArtifacts: 62,
    formatDescriptors: 8077,
    declarations: 19,
    artifactEvidence: 86,
    declarationTestFiles: 20,
    runtimeTestFiles: 196,
    tests: 216,
    passed: 216,
    failed: 0,
  });
});

test("all package names, public exports, entrypoints, and formats are exact", () => {
  assert.deepEqual(
    report.packages.map(entry => entry.name),
    inventory.packages.map(entry => entry.name),
  );
  for (const audited of inventory.packages) {
    const actual = report.packages.find(entry => entry.name === audited.name);
    assert(actual, audited.name);
    assert.equal(actual.status, "passed");
    assert.equal(actual.entrypoints, "passed");
    assert.equal(actual.publicExports, "passed");
    assert.deepEqual(actual.packageEntrypoints, audited.packageEntrypoints);
    assert.deepEqual(actual.publicExportNames, audited.publicExports);
    assert.ok(actual.declarationExportNames.length > 0, `${audited.name} declarations`);
    assertArtifact(actual.manifest);
    for (const target of actual.targets) assertArtifact(target);
    assert.equal(
      Object.keys(actual.runtimeDescriptors).length,
      actual.runtimeExportNames.length,
      `${audited.name} runtime descriptors`,
    );
  }

  const expectedFormats = inventory.packages.flatMap(entry =>
    entry.formats.map(format => `${entry.name}:${format}`)
  );
  assert.deepEqual(
    report.formats.map(entry => `${entry.package}:${entry.format}`),
    expectedFormats,
  );
  for (const row of report.formats) {
    assert.equal(row.status, "passed", `${row.package}:${row.format}`);
    assert.ok(row.artifacts.length > 0, `${row.package}:${row.format}`);
    for (const output of row.artifacts) {
      assertArtifact(output);
      assert.match(output.descriptorSha256, /^[a-f0-9]{64}$/u);
      assert.equal(Object.keys(output.descriptors).length, output.exports);
    }
  }
});

test("Vue emits all seven required formats and conventional artifacts", () => {
  const formats = report.formats
    .filter(entry => entry.package === "vue")
    .map(entry => entry.format)
    .sort();
  assert.deepEqual(formats, [
    "cjs",
    "esm-browser",
    "esm-browser-runtime",
    "esm-bundler",
    "esm-bundler-runtime",
    "global",
    "global-runtime",
  ]);
  const files = new Set(
    report.formats
      .filter(entry => entry.package === "vue")
      .flatMap(entry => entry.artifacts.map(output => output.path)),
  );
  for (const path of [
    "packages/vuelil/dist/vue.esm-bundler.js",
    "packages/vuelil/dist/vue.runtime.esm-bundler.js",
    "packages/vuelil/dist/vue.esm-browser.js",
    "packages/vuelil/dist/vue.runtime.esm-browser.js",
    "packages/vuelil/dist/vue.global.js",
    "packages/vuelil/dist/vue.runtime.global.js",
    "packages/vuelil/dist/vue.cjs.js",
  ]) {
    assert.ok(files.has(path), path);
  }
});

test("all declaration artifacts retain pinned provenance", () => {
  assert.equal(report.declarations.length, 19);
  for (const declaration of report.declarations) {
    assert.equal(declaration.exact, true);
    assertArtifact(declaration);
    assert.match(declaration.sourceSha256, /^[a-f0-9]{64}$/u);
    if (declaration.source.startsWith("node_modules/")) {
      assert.equal(
        declaration.sha256,
        sha256(readFileSync(resolve(root, declaration.source))),
        declaration.path,
      );
    }
  }
});

test("all 20 pinned declaration tests passed without modification", () => {
  const declarationTests = report.tests.filter(entry =>
    inventory.declarationTestFiles.includes(entry.path)
  );
  assert.equal(declarationTests.length, 20);
  assert.deepEqual(
    declarationTests.map(entry => entry.path),
    inventory.declarationTestFiles,
  );
  for (const entry of declarationTests) {
    const path = resolve(root, "upstream/vue", entry.path);
    const current = readFileSync(path);
    const pinned = execFileSync("git", ["show", `HEAD:${entry.path}`], {
      cwd: resolve(root, "upstream/vue"),
    });
    assert.equal(sha256(current), entry.sha256, entry.path);
    assert.equal(sha256(current), sha256(pinned), `${entry.path} is not unchanged`);
  }
});

test("CJS format artifacts expose their recorded exact runtime surfaces", () => {
  for (const row of report.formats.filter(entry => entry.format === "cjs")) {
    const packageEvidence = report.packages.find(entry => entry.name === row.package);
    const development = row.artifacts.find(entry => entry.production === false);
    const module = require(resolve(root, development.path));
    assert.deepEqual(Object.keys(module).sort(), packageEvidence.runtimeExportNames);
  }
});

test("generated JavaScript has no upstream implementation path imports", () => {
  for (const entry of report.artifacts.filter(({ path }) => /\.[cm]?js$/u.test(path))) {
    const source = readFileSync(resolve(root, entry.path), "utf8");
    assert.doesNotMatch(
      source,
      /(?:from|import\(|require\()\s*["'][^"']*(?:node_modules\/(?:vue|@vue)|upstream\/vue)[^"']*["']/u,
      entry.path,
    );
    assert.equal(source.includes(root), false, `${entry.path} contains an absolute workspace path`);
  }
});

test("scope advances only declaration, entrypoint, and format gates", () => {
  assert.equal(scope.gates.runtimeTestFilesPassed, 196);
  assert.equal(scope.gates.declarationTestFilesPassed, 20);
  assert.equal(scope.gates.declarations, "passed");
  assert.equal(scope.gates.packageEntrypointsPassed, 28);
  assert.equal(scope.gates.packageEntrypoints, "passed");
  assert.equal(scope.gates.formatRowsPassed, 38);
  assert.equal(scope.gates.packageFormats, "passed");
  assert.equal(scope.gates.candidatePassed, 216);
  assert.equal(scope.gates.projectSize, "failed");
  assert.equal(scope.gates.performance, "pending");
  assert.equal(scope.gates.pages, "pending");
});
