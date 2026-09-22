import assert from "node:assert/strict";
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { digest, fileIdentity, fingerprint, snapshotInputs, validateInvocation, validateMeasurements, validateTests } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";
import { runNodeTestEvidence } from "../../../finer/tools/node-test-evidence.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
assert.equal(process.argv.length, 3, "usage: qualify-marked-retained-open.mjs new-output-directory");
const output = resolve(process.argv[2]);
mkdirSync(output, { recursive: false });
const parentDirectory = join(directory, "marked-release-baseline");
const original = "/home/azureuser/markedlil";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const codec = "/tmp/lilscript-public-integration-release-baseline-20260919/lilscript-codec";
const inventoryPath = join(root, "benchmarks/libraries/markedlil.profiles.tests.json");
const fixturePath = join(root, "finer/tools/port-adapters/marked-profiles.test.mjs");
const files = ["test/compat.test.mjs", "test/options.test.mjs", "test/api.test.mjs"];
const identify = path => ({ path, ...fileIdentity(path) });
const json = path => JSON.parse(readFileSync(path, "utf8"));
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const evidencePaths = [
  fileURLToPath(import.meta.url), node, codec, inventoryPath, fixturePath, join(root, ".nvmrc"),
  ...["artifact-evidence", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts"].map(name => join(root, `finer/tools/${name}.mjs`)),
  ...["receipt.json", "markedlil.json", "manifest.json", "markedlil.profiles.tests.json", "source-before.json", "source-after.json", "dependencies-before.json", "dependencies-after.json", "parent-receipt.json", "parent-inputs-before.json", "parent-inputs-after.json"].map(name => join(parentDirectory, name)),
];
const pins = () => {
  const rows = evidencePaths.map(identify);
  return { files: rows, sha256: fingerprint(rows) };
};

// Verify every member and every directory listing of the previously frozen tree,
// including nested node_modules and symlink destinations. Source snapshots omit it.
function verifyDependencies(expected) {
  assert.equal(fingerprint(expected.files), expected.sha256);
  assert.equal(expected.root, join(original, "node_modules"));
  const children = new Map();
  for (const row of expected.files.filter(row => row.path)) {
    const parent = dirname(row.path) === "." ? "" : dirname(row.path);
    if (!children.has(parent)) children.set(parent, []);
    children.get(parent).push(basename(row.path));
  }
  const rows = expected.files.map(row => {
    const path = join(expected.root, row.path);
    const info = statSync(path);
    const actual = { path: row.path, kind: info.isDirectory() ? "directory" : "file" };
    if (lstatSync(path).isSymbolicLink()) Object.assign(actual, { symlink: readlinkSync(path), resolved: realpathSync(path) });
    if (info.isDirectory()) assert.deepEqual(readdirSync(path).sort(), (children.get(row.path) ?? []).sort(), `dependency members changed: ${path}`);
    else {
      assert(info.isFile(), `unsupported dependency: ${path}`);
      Object.assign(actual, { executable: Boolean(info.mode & 0o111), ...fileIdentity(path) });
    }
    assert.deepEqual(actual, row, `dependency changed: ${path}`);
    return actual;
  });
  return { root: expected.root, files: rows, sha256: fingerprint(rows) };
}

const receipt = {
  schema: 1, kind: "marked-retained-open-artifact-supplement", started: new Date().toISOString(),
  passed: false, qualification: "unverified", maintainedBoundaryPassed: false, commands: [],
  scope: "Existing complete Brotli/open artifact, unchanged packaging-only path and three unchanged original test files; not a fresh compiler build or full baseline retry.",
  limitations: [
    "The original four-profile build failed and its receipt remains unchanged. No closed/gzip/raw-profile artifact is substituted or credited.",
    "The selected 29 frozen suite/test nodes are a supplement, not a replacement for all 75 required nodes or additional declared coverage. Original corpus exclusions and inline-input filters remain unchanged.",
    "The original tests execute VM-loaded UMD, but the existing observer binds only ESM/CJS module loads. Exact scored UMD execution, browser and installed-package coverage remain open.",
    "The upstream marked package is used by the unchanged assertions as an oracle, not substituted for either candidate delivery. The upstream-only official-parse file earns no candidate credit.",
    "ESM and CJS files are scored independently with the preserved canonical codec. These are file metrics, not independent objective winners, a competitor comparison or a complete frozen deployment-cost matrix.",
    "This artifact uses the older source-pinned legacy backend. Current compiler inputs, semantic-backend support, speed improvements and D2/D5 decisions are not qualified here.",
    "Dependencies are reused offline with complete existing tree identities verified before and after, not claimed hermetic. Supervision bounds process groups, not hard OS time/RSS or escaped sessions.",
  ],
};
let initialPins, beforeOriginal, expectedSource, expectedDependencies, workspace, beforeWorkspace, packaged;
const overrides = {
  PATH: "/home/azureuser/.nvm/versions/node/v24.11.1/bin:/usr/local/bin:/usr/bin:/bin",
  NODE_OPTIONS: "", NODE_PATH: "", npm_config_offline: "true", npm_config_ignore_scripts: "true",
};
Object.assign(process.env, overrides);
delete process.env.NODE_TEST_CONTEXT;
delete process.env.ESBUILD_BINARY_PATH;
receipt.environment = { overrides, otherwise: "inherited", removed: ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"] };
async function run(label, command, args, cwd, timeoutMs) {
  const result = await runBoundedCommand(command, args, { cwd, env: process.env, timeoutMs, maxBuffer: 16 * 1024 * 1024 });
  writeFileSync(join(output, `${label}.stdout`), result.stdout);
  writeFileSync(join(output, `${label}.stderr`), result.stderr);
  receipt.commands.push({ label, command, args, cwd, status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision });
  save("receipt.json", receipt);
  assert.equal(result.error, undefined, `${label}: command failed`);
  assert.equal(result.signal, null, `${label}: command signaled`);
  assert.equal(result.status, 0, `${label}: nonzero exit; see logs`);
  return result;
}

try {
  initialPins = pins();
  save("inputs-before.json", initialPins);
  copyFileSync(fileURLToPath(import.meta.url), join(output, "wrapper.mjs"));
  assert.equal(fileIdentity(process.execPath).sha256, fileIdentity(node).sha256);
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  const parentPath = join(parentDirectory, "receipt.json");
  assert.equal(fileIdentity(parentPath).sha256, "b3d659bf1a9eedd6f1520ab25b222b69698c0d4620b80d6108b665fdcda4d52f");
  const parent = json(parentPath), build = json(join(parentDirectory, "markedlil.json"));
  assert.equal(parent.passed, false);
  assert.equal(parent.inputsStable, true);
  assert.equal(parent.build.supervision.timedOut, true);
  for (const path of evidencePaths.filter(path => dirname(path) === parentDirectory && path !== parentPath)) {
    const frozen = parent.outputs.find(row => row.path === basename(path));
    assert(frozen, `parent output identity missing: ${path}`);
    assert.deepEqual(fileIdentity(path), { sha256: frozen.sha256, bytes: frozen.bytes });
  }
  assert.deepEqual(fileIdentity(inventoryPath), fileIdentity(join(parentDirectory, "markedlil.profiles.tests.json")));
  assert.equal(build.build.invocations.length, 1);
  const invocation = build.build.invocations[0];
  assert.deepEqual(parent.build.invocations, build.build.invocations);
  assert.deepEqual(validateInvocation(invocation, { compilerSha256: parent.parent.compiler.sha256, contractSha256: build.contractSha256, verifyOutput: true }), []);
  assert.deepEqual(invocation.output, { path: join(build.workspace, "dist/marked.raw.js"), previous: null, sha256: "c08107d23ed741a987e8d9297f8f93594749c57323b1f03e7467b399120621de", bytes: 34092 });
  assert.deepEqual(fileIdentity(codec), { sha256: parent.parent.codec.sha256, bytes: parent.parent.codec.bytes });
  const compilerParent = json(join(parentDirectory, "parent-receipt.json"));
  assert.equal(fileIdentity(join(parentDirectory, "parent-receipt.json")).sha256, parent.parent.sha256);
  assert.equal(compilerParent.passed, true);
  assert.equal(compilerParent.inputsStable, true);
  assert.equal(compilerParent.profile, "release");
  assert.equal(compilerParent.inputSha256, parent.parent.inputSha256);
  for (const name of ["parent-inputs-before.json", "parent-inputs-after.json"]) {
    const snapshot = json(join(parentDirectory, name));
    assert.equal(digest(JSON.stringify(snapshot.files)), snapshot.sha256);
    assert.equal(snapshot.sha256, compilerParent.inputSha256);
  }
  assert(compilerParent.binaries.some(row => row.sha256 === parent.parent.compiler.sha256));
  assert(compilerParent.binaries.some(row => row.sha256 === parent.parent.codec.sha256));
  receipt.parent = { ...identify(parentPath), compilerInputSha256: compilerParent.inputSha256, invocation, codec: identify(codec) };
  expectedSource = json(join(parentDirectory, "source-before.json"));
  assert.equal(fingerprint(expectedSource.files), expectedSource.sha256);
  assert.deepEqual(expectedSource.files, json(join(parentDirectory, "source-after.json")).files);
  assert.deepEqual(expectedSource.files, build.source.snapshot.files);
  assert.deepEqual(expectedSource.files, snapshotInputs(original).files);
  const sourceInputs = invocation.inputs.filter(file => file.path !== ".lilscript-test-adapter/suite.test.mjs");
  assert.deepEqual(sourceInputs, expectedSource.files);
  assert.equal(invocation.inputs.length, expectedSource.files.length + 1);
  const adapter = invocation.inputs.find(file => file.path === ".lilscript-test-adapter/suite.test.mjs");
  assert.deepEqual({ sha256: adapter.sha256, bytes: adapter.bytes }, fileIdentity(fixturePath));
  assert.equal(build.contractSha256, fingerprint({ source: expectedSource.sha256, build: "scripts/build.mjs --compile", outputRoots: ["dist"] }));
  assert.equal(invocation.configuration.sha256, fileIdentity(join(original, "lilscript.toml")).sha256);
  beforeOriginal = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
  save("original-before.json", beforeOriginal);
  expectedDependencies = json(join(parentDirectory, "dependencies-before.json"));
  assert.deepEqual(expectedDependencies, json(join(parentDirectory, "dependencies-after.json")));
  save("dependencies-before.json", verifyDependencies(expectedDependencies));
  workspace = join(mkdtempSync(join(tmpdir(), "lilscript-marked-retained-open-20260919-")), "markedlil");
  mkdirSync(workspace);
  for (const file of expectedSource.files) {
    assert.equal(file.kind, "file");
    const destination = join(workspace, file.path);
    mkdirSync(dirname(destination), { recursive: true });
    cpSync(join(original, file.path), destination);
  }
  beforeWorkspace = snapshotInputs(workspace);
  assert.deepEqual(beforeWorkspace.files, expectedSource.files);
  save("workspace-before.json", beforeWorkspace);
  symlinkSync(expectedDependencies.root, join(workspace, "node_modules"), "dir");
  assert.equal(realpathSync(join(workspace, "node_modules")), realpathSync(expectedDependencies.root));
  mkdirSync(join(workspace, "dist"));
  copyFileSync(invocation.output.path, join(workspace, "dist/marked.raw.js"));
  assert.deepEqual(readdirSync(join(workspace, "dist")), ["marked.raw.js"]);
  Object.assign(process.env, { LILSCRIPT_ROOT: join(workspace, "no-compiler"), LILSCRIPT_COMPILER: join(workspace, "no-compiler/lilscript") });
  Object.assign(overrides, { LILSCRIPT_ROOT: process.env.LILSCRIPT_ROOT, LILSCRIPT_COMPILER: process.env.LILSCRIPT_COMPILER });
  assert.equal(existsSync(process.env.LILSCRIPT_ROOT), false);
  receipt.workspace = workspace;
  receipt.runtime = { ...identify(node), version: process.version };
  await run("packaging", node, ["scripts/build.mjs"], workspace, 60_000);
  assert.deepEqual(readdirSync(join(workspace, "dist")).sort(), ["marked.cjs", "marked.d.ts", "marked.esm.js", "marked.raw.js", "marked.umd.js"]);
  assert.deepEqual(fileIdentity(join(workspace, "dist/marked.raw.js")), { sha256: invocation.output.sha256, bytes: invocation.output.bytes });
  const banner = "/*! @itslil/marked 18.0.10 | LilScript reimplementation of marked 18.0.10 | MIT */\n";
  assert.equal(readFileSync(join(workspace, "dist/marked.esm.js"), "utf8"), `${banner}${readFileSync(invocation.output.path, "utf8").trimEnd()}\n`);
  assert.deepEqual(snapshotInputs(workspace), beforeWorkspace);
  packaged = snapshotInputs(join(workspace, "dist"));
  save("packaged-before.json", packaged);
  const inventory = json(inventoryPath);
  assert.equal(inventory.requiredCases.length, 75);
  assert.equal(new Set(inventory.requiredCases).size, 75);
  const requiredCases = inventory.requiredCases.filter(id => files.includes(JSON.parse(id)[0]));
  assert.equal(requiredCases.length, 29);
  receipt.requiredCases = requiredCases;
  receipt.remainingRequiredCases = inventory.requiredCases.filter(id => !requiredCases.includes(id));
  receipt.additionalRequiredCoverage = inventory.additionalRequiredCoverage;
  const artifacts = ["dist/marked.esm.js", "dist/marked.cjs"];
  const report = await runNodeTestEvidence({ cwd: workspace, files, artifactPaths: artifacts, requiredCases,
    requiredTestFiles: inventory.testFiles.filter(row => files.includes(row.path)), requiredFixtures: inventory.fixtures,
    directory: join(output, "tests"), node, timeoutMs: 60_000 });
  receipt.tests = { ...identify(join(output, "tests/report.json")), certification: report.certification, errors: report.errors };
  save("receipt.json", receipt);
  assert.equal(report.evidence.cases.length, requiredCases.length);
  assert.deepEqual(report.evidence.cases.map(row => row.id).sort(), [...requiredCases].sort());
  assert.deepEqual(validateTests(report.evidence), []);
  assert.deepEqual(report.errors, []);
  const paths = artifacts.map(path => join(workspace, path));
  const measured = await run("codec", codec, ["--json", ...paths], workspace, 60_000);
  const document = JSON.parse(measured.stdout);
  const scores = validateMeasurements(document, paths);
  receipt.artifacts = [...scores].map(([path, sizes]) => ({ ...identify(path), ...sizes }));
  receipt.passed = true;
  receipt.qualification = "verified-retained-open-supplement-only";
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const finalPins = pins();
    save("inputs-after.json", finalPins);
    receipt.inputsStable = initialPins?.sha256 === finalPins.sha256;
    assert.equal(receipt.inputsStable, true);
    if (beforeOriginal) {
      const after = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
      save("original-after.json", after);
      assert.deepEqual(after, beforeOriginal);
    }
    if (expectedDependencies) save("dependencies-after.json", verifyDependencies(expectedDependencies));
    if (beforeWorkspace) {
      const after = snapshotInputs(workspace);
      save("workspace-after.json", after);
      assert.deepEqual(after, beforeWorkspace);
    }
    if (packaged) {
      const after = snapshotInputs(join(workspace, "dist"));
      save("packaged-after.json", after);
      assert.deepEqual(after, packaged);
    }
  } catch (error) {
    receipt.passed = false;
    receipt.qualification = "unverified";
    receipt.validationFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  try {
    if (workspace && existsSync(join(workspace, "dist"))) cpSync(join(workspace, "dist"), join(output, "artifacts"), { recursive: true });
  } catch (error) {
    receipt.passed = false;
    receipt.qualification = "unverified";
    receipt.archiveFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.completed = new Date().toISOString();
  receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files;
  save("receipt.json", receipt);
  process.stdout.write(`${JSON.stringify({ output, passed: receipt.passed, qualification: receipt.qualification, failure: receipt.failure?.message, validationFailure: receipt.validationFailure?.message, artifacts: receipt.artifacts })}\n`);
}
