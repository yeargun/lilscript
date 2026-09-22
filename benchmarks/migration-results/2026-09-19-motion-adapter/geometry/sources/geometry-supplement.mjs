import assert from "node:assert/strict";
import { copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs } from "../../../finer/tools/artifact-evidence.mjs";
import { runNodeTestEvidence } from "../../../finer/tools/node-test-evidence.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
const output = join(here, "geometry"), parentDirectory = join(here, "existing-dist");
const read = path => JSON.parse(readFileSync(path));
const identify = path => ({ path, ...fileIdentity(path) });
const parent = read(join(parentDirectory, "receipt.json"));
const before = read(join(parentDirectory, "before.json"));
const testPath = join(here, "geometry.test.mjs");
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
mkdirSync(output);
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const started = performance.now();
const removed = Object.keys(process.env).filter(key => /^(?:LILSCRIPT_|MOTIONLIL_|npm_)/i.test(key) || ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
for (const key of removed) delete process.env[key];
const overrides = { PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "" };
Object.assign(process.env, overrides);
const receipt = {
  schema: 1, kind: "motion-public-geometry-observation", passed: false,
  started: new Date().toISOString(), sourceBuilt: false,
  parent: identify(join(parentDirectory, "receipt.json")),
  environment: { removed, overrides },
  limits: { sharedAttemptMs: 30_000, externalSupervisorMs: 60_000 },
  scope: "Five new independent observation tests against upstream and retained candidate, not original-suite identities, a compiler repair or a D2 decision",
};
save("receipt.json", receipt);

function verifyParent() {
  assert(parent.passed && parent.inputsStable);
  for (const file of parent.outputs) assert.deepEqual(fileIdentity(join(parentDirectory, file.path)), { sha256: file.sha256, bytes: file.bytes });
  for (const file of before.inputs.files) assert.deepEqual(fileIdentity(file.path), { sha256: file.sha256, bytes: file.bytes });
  assert.deepEqual(before, read(join(parentDirectory, "after.json")));
  assert.deepEqual(snapshotInputs(parent.original), before.source);
  for (const [directory, snapshot] of [[parent.original, before.original], [parent.workspace, before.workspace]]) {
    assert.deepEqual(snapshotInputs(directory, { exclude: [".git", "node_modules", "target", ".cache"] }), snapshot);
  }
  assert.equal(fingerprint(before.project.files), before.project.sha256);
  for (const entry of before.project.files) {
    const path = join(before.project.root, entry.path), info = statSync(path);
    assert.equal(info.isDirectory(), entry.kind === "directory");
    assert.equal(lstatSync(path).isSymbolicLink(), Boolean(entry.symlink));
    if (entry.symlink) {
      assert.equal(readlinkSync(path), entry.symlink);
      assert.equal(realpathSync(path), entry.resolved);
    }
    if (entry.kind === "file") {
      assert.deepEqual(fileIdentity(path), { sha256: entry.sha256, bytes: entry.bytes });
      assert.equal(Boolean(info.mode & 0o111), entry.executable);
    } else {
      const children = before.project.files.filter(file => file.path && (dirname(file.path) === "." ? "" : dirname(file.path)) === entry.path)
        .map(file => file.path.slice(entry.path ? entry.path.length + 1 : 0)).sort();
      assert.deepEqual(readdirSync(path).sort(), children);
    }
  }
}

try {
  assert.equal(process.version, "v24.11.1");
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  verifyParent();
  const toolPaths = ["artifact-evidence", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts"].map(name => join(root, `finer/tools/${name}.mjs`));
  receipt.inputs = [fileURLToPath(import.meta.url), testPath, node, ...toolPaths].map(identify);
  save("inputs.json", receipt.inputs);
  mkdirSync(join(output, "sources"));
  for (const input of receipt.inputs.filter(input => input.path.endsWith(".mjs"))) copyFileSync(input.path, join(output, "sources", basename(input.path)));
  const files = [relative(root, testPath)];
  const report = await runNodeTestEvidence({
    cwd: root, files, requiredFilePatterns: files,
    artifactPaths: [join(parent.workspace, "dist/full.js"), join(parent.dependencies, "motion/dist/es/index.mjs")],
    requiredCases: [], requiredTestFiles: [{ path: files[0], ...fileIdentity(testPath) }],
    requiredFixtures: [receipt.parent], directory: join(output, "node"),
    timeoutMs: Math.max(1, Math.floor(30_000 - (performance.now() - started))),
  });
  receipt.report = identify(join(output, "node/report.json"));
  receipt.cases = report.evidence.cases;
  receipt.errors = report.errors;
  receipt.passed = report.evidence.exitCode === 0 && receipt.cases.length === 10 && receipt.cases.every(row => row.status === "pass")
    && report.errors.length === 1 && report.errors[0] === "required test inventory missing or ambiguous";
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
} finally {
  try {
    verifyParent();
    for (const input of receipt.inputs ?? []) assert.deepEqual(fileIdentity(input.path), { sha256: input.sha256, bytes: input.bytes });
    receipt.inputsStable = true;
  } catch (error) {
    receipt.inputsStable = false;
    receipt.passed = false;
    receipt.validationFailure = { message: error.message, stack: error.stack };
  }
  receipt.elapsedMs = performance.now() - started;
  receipt.completed = new Date().toISOString();
  receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files;
  save("receipt.json", receipt);
  console.log(JSON.stringify({ output, passed: receipt.passed, inputsStable: receipt.inputsStable, cases: receipt.cases?.length, failure: receipt.failure?.message }));
  if (!receipt.passed) process.exitCode = 1;
}
