import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs, validateBuild } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
assert.equal(process.argv.length, 3, "usage: qualify-package-cjs.mjs new-output-directory (relative to this script directory)");
const output = resolve(directory, process.argv[2]);
mkdirSync(output, { recursive: false });
const workspace = "/tmp/lilscript-source-built-hast-20260919/workspaces/hast-util-to-htmllil";
const original = "/home/azureuser/hast-util-to-htmllil";
const nodeRoot = "/home/azureuser/.nvm/versions/node/v24.11.1";
const node = join(nodeRoot, "bin/node");
const npmRoot = join(nodeRoot, "lib/node_modules/npm");
const npm = join(npmRoot, "bin/npm-cli.js");
const accepted = join(root, "benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-19T13-05-45.258Z");
const buildRecordPath = join(directory, "source-built/hast-util-to-htmllil.json");
const buildManifestPath = join(directory, "source-built/manifest.json");
const additional = join(directory, "source-built-additional");
const probe = join(directory, "package-cjs-smoke.mjs");
const expectedCjs = "6251b2367acc1a9875fe44fc3ff78cb622056ae1551dbdcfd4d560d4c4e659fb";
const identify = path => ({ path, ...fileIdentity(path) });
const json = path => JSON.parse(readFileSync(path, "utf8"));
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const sourceSnapshot = path => snapshotInputs(path, { exclude: [".git", "node_modules", "target", ".cache"] });
const evidencePaths = [
  fileURLToPath(import.meta.url), probe, node, join(npmRoot, "package.json"), npm, join(npmRoot, "lib/cli.js"),
  join(root, ".nvmrc"), ...["artifact-evidence.mjs", "observe-node-artifacts.mjs", "bounded-command.mjs"].map(name => join(root, "finer/tools", name)),
  buildRecordPath, buildManifestPath, join(directory, "source-built/tests/hast-util-to-htmllil/report.json"),
  join(directory, "public-boundary.json"), join(additional, "receipt.json"), join(additional, "workspace.after.json"),
  ...["receipt.json", "inputs-before.json", "inputs-after.json"].map(name => join(accepted, name)),
];
const pins = () => {
  const files = evidencePaths.map(identify);
  return { files, sha256: fingerprint(files) };
};
const requiredCases = ["package-resolution", "named-exports", "public-name", "public-arity", "text", "escaping", "invalid-argument", "array-input"];
const receipt = {
  schema: 1, started: new Date().toISOString(), passed: false, qualification: "unverified",
  kind: "installed-package-cjs-supplement", commands: [], requiredCases,
  scope: "unchanged source-built legacy HAST delivery, offline local tarball consumer, eight supplementary cases",
  limitations: [
    "These supplementary cases do not replace the original 460 identities or establish complete CJS API coverage.",
    "Public name is required to remain toHtml; an observed legacy name is not accepted as a replacement contract.",
    "The original h('b') and h('i') array fixture is expanded into equivalent plain HAST element objects; no upstream or hastscript package is loaded by this consumer.",
    "Package creation and installation explicitly disable lifecycle scripts; earlier unchanged check:pack/check:site evidence remains separate.",
    "No library/compiler rebuild, compiler/library source edit, network dependency installation or new codec measurement is performed.",
    "UMD/browser, declaration consumers, constructibility and complete dependency/delivery coverage remain open; no D2/D5 or semantic-backend claim.",
    "Source snapshots exclude .git, node_modules, target and .cache; installed published files are compared with the pack inventory and Node/npm tool identities are recorded.",
  ],
};
let initialPins, beforeWorkspace, beforeOriginal, installedBefore;
let consumer;
async function run(label, command, args, options = {}) {
  const started = process.hrtime.bigint();
  const result = await runBoundedCommand(command, args, { cwd: options.cwd ?? root, env, timeoutMs: 60_000, maxBuffer: 16 * 1024 * 1024 });
  writeFileSync(join(output, `${label}.stdout`), result.stdout);
  writeFileSync(join(output, `${label}.stderr`), result.stderr);
  const record = {
    label, command, args, cwd: options.cwd ?? root, status: result.status, signal: result.signal,
    error: result.error ? { code: result.error.code, message: result.error.message } : null,
    wall_ns: Number(process.hrtime.bigint() - started), supervision: result.supervision,
    stdout: identify(join(output, `${label}.stdout`)), stderr: identify(join(output, `${label}.stderr`)),
  };
  receipt.commands.push(record);
  save("receipt.json", receipt);
  assert.equal(result.error, undefined, `${label}: ${result.error?.message}`);
  assert.equal(result.signal, null, `${label}: signal`);
  if (!options.collectFailure) assert.equal(result.status, 0, `${label}: failed; see saved logs`);
  return result;
}
let env;
try {
  initialPins = pins();
  save("inputs-before.json", initialPins);
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.equal(fileIdentity(process.execPath).sha256, fileIdentity(node).sha256);
  beforeWorkspace = sourceSnapshot(workspace);
  beforeOriginal = sourceSnapshot(original);
  save("workspace-before.json", beforeWorkspace);
  save("original-before.json", beforeOriginal);
  const expectedWorkspace = json(join(additional, "workspace.after.json"));
  assert.equal(fingerprint(expectedWorkspace.files), expectedWorkspace.sha256);
  assert.deepEqual(beforeWorkspace.files, expectedWorkspace.files, "preserved source-built workspace changed since original checks");
  assert.equal(fileIdentity(buildRecordPath).sha256, "0a3c34ff4c96f77ba1f8fb34dbaa3099431567cbc9837c2dc47224c3f5b7d675");
  assert.equal(fileIdentity(buildManifestPath).sha256, "e9e256873f6138e17b25641d44323495d5b3f66daf2fa0aa41b67c3a81c3ba99");
  assert.equal(fileIdentity(join(additional, "receipt.json")).sha256, "753f8619613608a88532a7e230dbeb8fbaf8e8219f813d512162f3b480ecc8e0");
  const built = json(buildRecordPath);
  const manifest = json(buildManifestPath);
  assert.equal(built.build.trust, "ok");
  assert.equal(built.tests.status, 0);
  assert.equal(built.workspace, workspace);
  assert.deepEqual(validateBuild({ exitCode: built.build.status, invocations: built.build.invocations,
    compilerSha256: manifest.compiler.sha256, contractSha256: built.contractSha256, verifyOutputs: true }), []);
  const compilerReceipt = json(join(accepted, "receipt.json"));
  assert.equal(compilerReceipt.passed, true);
  assert.equal(compilerReceipt.inputsStable, true);
  for (const name of ["inputs-before.json", "inputs-after.json"]) {
    const snapshot = json(join(accepted, name));
    assert.equal(createHash("sha256").update(JSON.stringify(snapshot.files)).digest("hex"), snapshot.sha256);
    assert.equal(snapshot.sha256, compilerReceipt.inputSha256);
  }
  assert(compilerReceipt.binaries.some(binary => binary.sha256 === manifest.compiler.sha256));
  assert(compilerReceipt.binaries.some(binary => binary.sha256 === manifest.codec.sha256));
  for (const file of built.source.snapshot.files.filter(file => !file.path.startsWith("_site/"))) {
    assert.deepEqual(beforeWorkspace.files.find(current => current.path === file.path), file, `source build input changed: ${file.path}`);
  }
  for (const artifact of built.artifacts) assert.equal(fileIdentity(join(workspace, artifact.path)).sha256, artifact.sha256);
  assert.equal(fileIdentity(join(workspace, "dist/to-html.cjs")).sha256, expectedCjs);
  assert.deepEqual(fileIdentity(join(workspace, "dist/to-html.d.ts")), fileIdentity(join(workspace, "types/to-html.d.ts")));
  const publicBoundary = json(join(directory, "public-boundary.json"));
  assert.equal(publicBoundary.sourceBuildRecordSha256, fileIdentity(buildRecordPath).sha256);
  const upstream = publicBoundary.records.find(record => record.label === "upstream").exports[0];
  assert.equal(upstream.name, "toHtml");
  assert.equal(upstream.length, 2);
  receipt.parents = {
    sourceBuild: identify(buildRecordPath), buildManifest: identify(buildManifestPath),
    compilerQualification: identify(join(accepted, "receipt.json")), compilerInputsSha256: compilerReceipt.inputSha256,
    originalSuite: identify(join(directory, "source-built/tests/hast-util-to-htmllil/report.json")),
    originalAdditionalChecks: identify(join(additional, "receipt.json")), publicBoundary: identify(join(directory, "public-boundary.json")),
  };
  receipt.caseOrigins = {
    "named-exports": { file: "test/to-html.test.mjs", line: 19 },
    text: { file: "test/official/text.js", line: 8 }, escaping: { file: "test/official/text.js", line: 11 },
    "invalid-argument": { file: "test/official/core.js", line: 13 },
    "array-input": { file: "test/official/core.js", line: 31, fixture: "explicit HAST equivalent of h('b'), h('i')" },
    "public-name": { receipt: "public-boundary.json", record: "upstream", expected: "toHtml" },
    "public-arity": { receipt: "public-boundary.json", record: "upstream", expected: 2 },
    "package-resolution": { file: "package.json", field: "exports...require" },
  };
  const scratch = mkdtempSync(join(tmpdir(), "lilscript-hast-package-cjs-20260919-"));
  consumer = join(scratch, "consumer");
  const packed = join(output, "packed");
  mkdirSync(consumer);
  mkdirSync(packed);
  const overrides = {
    PATH: `${join(nodeRoot, "bin")}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "",
    npm_config_offline: "true", npm_config_ignore_scripts: "true", npm_config_audit: "false", npm_config_fund: "false",
    npm_config_cache: join(scratch, "npm-cache"), npm_config_userconfig: join(scratch, "empty.npmrc"),
  };
  writeFileSync(overrides.npm_config_userconfig, "");
  env = { ...process.env, ...overrides };
  delete env.NODE_TEST_CONTEXT;
  receipt.environment = { overrides, otherwise: "inherited", removed: ["NODE_TEST_CONTEXT"] };
  receipt.consumer = consumer;
  receipt.runtime = { ...identify(node), version: process.version, npmCli: identify(npm), npmVersion: json(join(npmRoot, "package.json")).version };
  for (const [path, name] of [[fileURLToPath(import.meta.url), "wrapper.mjs"], [probe, "smoke.mjs"], [buildRecordPath, "source-build.json"], [buildManifestPath, "source-build-manifest.json"]]) copyFileSync(path, join(output, name));
  const packRun = await run("pack", node, [npm, "pack", workspace, "--offline", "--ignore-scripts", "--json", "--pack-destination", packed], { cwd: scratch });
  const packRows = JSON.parse(packRun.stdout.toString("utf8"));
  assert.equal(packRows.length, 1);
  const pack = packRows[0];
  assert.equal(pack.name, "@itslil/hast-util-to-html");
  assert.equal(pack.version, "9.0.7");
  assert.equal(pack.filename, "itslil-hast-util-to-html-9.0.7.tgz");
  assert.equal(pack.files.length, 26, "original package inventory changed");
  const tarball = join(packed, pack.filename);
  assert.equal(pack.integrity, `sha512-${createHash("sha512").update(readFileSync(tarball)).digest("base64")}`);
  const packageFiles = pack.files.map(file => {
    const source = beforeWorkspace.files.find(row => row.path === file.path);
    assert(source && source.kind === "file", `unexpected packaged file: ${file.path}`);
    assert.equal(file.size, source.bytes);
    return { path: source.path, sha256: source.sha256, bytes: source.bytes };
  }).sort((a, b) => a.path.localeCompare(b.path));
  receipt.tarball = identify(tarball);
  receipt.packagedFiles = packageFiles;
  receipt.packagedManifest = identify(join(workspace, "package.json"));
  assert.deepEqual(json(join(workspace, "package.json")).dependencies ?? {}, {});
  await run("install", node, [npm, "install", "--prefix", consumer, "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--package-lock=false", "--omit=dev", "--json", tarball], { cwd: scratch });
  const installed = join(consumer, "node_modules/@itslil/hast-util-to-html");
  installedBefore = sourceSnapshot(installed);
  assert.deepEqual(installedBefore.files.map(({ path, sha256, bytes }) => ({ path, sha256, bytes })).sort((a, b) => a.path.localeCompare(b.path)), packageFiles);
  assert.deepEqual(fileIdentity(join(installed, "package.json")), fileIdentity(join(workspace, "package.json")));
  assert.equal(fileIdentity(join(installed, "dist/to-html.cjs")).sha256, expectedCjs);
  assert.deepEqual(readdirSync(join(consumer, "node_modules")).filter(name => !name.startsWith(".")), ["@itslil"]);
  assert.deepEqual(readdirSync(join(consumer, "node_modules/@itslil")), ["hast-util-to-html"]);
  save("installed-before.json", installedBefore);
  copyFileSync(join(installed, "package.json"), join(output, "installed-package.json"));
  copyFileSync(join(workspace, "package.json"), join(output, "packaged-package.json"));
  receipt.installedManifest = identify(join(installed, "package.json"));
  const smokeResult = join(output, "smoke-results.json");
  const loadDirectory = join(output, "loaded");
  const smoke = await run("smoke", node, [probe, consumer, loadDirectory, smokeResult], { cwd: consumer, collectFailure: true });
  const result = json(smokeResult);
  assert.deepEqual(result.cases.map(row => row.id), requiredCases);
  receipt.cases = result.cases;
  receipt.observed = result.observed;
  receipt.dependencyClosure = { passed: result.dependencyClosure, loadedCommonJsFiles: result.loadedCommonJsFiles };
  assert.equal(result.dependencyClosure, true, "installed consumer loaded unexpected CommonJS dependencies");
  const loaded = readdirSync(loadDirectory).map(name => json(join(loadDirectory, name)));
  assert.equal(loaded.length, 1, "expected one actual CommonJS production load");
  assert.equal(loaded[0].kind, "node-module-load");
  assert.equal(loaded[0].path, join(installed, "dist/to-html.cjs"));
  assert.equal(loaded[0].format, "commonjs");
  assert.equal(loaded[0].sha256, expectedCjs);
  assert.equal(loaded[0].bytes, 31100);
  receipt.loadedArtifacts = loaded;
  receipt.behavioralSmokePassed = result.cases.filter(row => row.id !== "public-name").every(row => row.status === "pass");
  receipt.publicNamePreserved = result.cases.find(row => row.id === "public-name").status === "pass";
  receipt.passed = result.passed && smoke.status === 0;
  receipt.completedAllCases = true;
  if (!receipt.passed) {
    receipt.failure = { message: "One or more required supplementary cases failed; expectations and artifacts remain unchanged" };
    process.exitCode = 1;
  }
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const finalPins = pins();
    save("inputs-after.json", finalPins);
    receipt.inputsStable = initialPins?.sha256 === finalPins.sha256;
    assert.equal(receipt.inputsStable, true);
    for (const [name, path, before] of [["workspace", workspace, beforeWorkspace], ["original", original, beforeOriginal]]) {
      const after = sourceSnapshot(path);
      save(`${name}-after.json`, after);
      receipt[`${name}Unchanged`] = before?.sha256 === after.sha256;
      assert.equal(receipt[`${name}Unchanged`], true);
    }
    if (installedBefore) {
      const after = sourceSnapshot(installedBefore.root);
      save("installed-after.json", after);
      receipt.installedUnchanged = installedBefore.sha256 === after.sha256;
      assert.equal(receipt.installedUnchanged, true);
    }
    if (receipt.tarball) assert.equal(fileIdentity(receipt.tarball.path).sha256, receipt.tarball.sha256);
  } catch (error) {
    receipt.passed = false;
    receipt.validationFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.completed = new Date().toISOString();
  receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files;
  save("receipt.json", receipt);
  console.log(JSON.stringify({ directory: output, passed: receipt.passed, completedAllCases: receipt.completedAllCases ?? false, observed: receipt.observed, inputsStable: receipt.inputsStable }));
}
