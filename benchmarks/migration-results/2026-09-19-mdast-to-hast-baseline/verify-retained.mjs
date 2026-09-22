import assert from "node:assert/strict";
import { copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs, validateBuild, validateMeasurements, validateTests } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const builtDirectory = join(here, "source-built-release"), output = join(here, "postrun-verification");
mkdirSync(output, { recursive: false });
const read = path => JSON.parse(readFileSync(path, "utf8"));
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const identity = path => ({ path, ...fileIdentity(path) });
const builtReceiptPath = join(builtDirectory, "receipt.json");
const receipt = { schema: 1, kind: "failed-mdast-baseline-retained-evidence-verification", started: new Date().toISOString(), passed: false, originalTestBoundaryPassed: false, qualification: "unverified", scope: "Rehash frozen failed evidence, current input closures and exact loaded bytes; one independent canonical codec replay on retained artifacts. No compiler or library test rerun." };
save("receipt.json", receipt);

function dependencyTree(directory) {
  const files = [];
  let bytes = 0;
  function walk(path, ancestors = new Set()) {
    const actual = realpathSync(path), info = statSync(path);
    assert(!ancestors.has(actual), `dependency symlink cycle: ${path}`);
    const entry = { path: relative(directory, path), kind: info.isDirectory() ? "directory" : "file" };
    if (lstatSync(path).isSymbolicLink()) Object.assign(entry, { symlink: readlinkSync(path), resolved: actual });
    if (info.isDirectory()) {
      files.push(entry);
      const parents = new Set([...ancestors, actual]);
      for (const name of readdirSync(path).sort()) walk(join(path, name), parents);
    } else {
      assert(info.isFile(), `unsupported dependency input: ${path}`);
      bytes += info.size;
      assert(bytes <= 512 * 1024 * 1024, "dependency snapshot exceeds 512 MiB cap");
      files.push({ ...entry, executable: Boolean(info.mode & 0o111), ...fileIdentity(path) });
    }
    assert(files.length <= 50_000, "dependency snapshot exceeds 50,000-entry cap");
  }
  walk(directory);
  return { root: directory, files, bytes, sha256: fingerprint(files) };
}

let pins;
try {
  assert.equal(fileIdentity(builtReceiptPath).sha256, "b2c45af4b4bdf370f94383b936b8a56bd175ff342e5cbf8eb64b1abc1bd26cce");
  const builtReceipt = read(builtReceiptPath), built = read(join(builtDirectory, "mdast-util-to-hastlil.json"));
  assert.equal(builtReceipt.passed, false);
  assert.equal(builtReceipt.originalTestBoundaryPassed, false);
  assert.equal(builtReceipt.inputsStable, true);
  for (const file of builtReceipt.outputs) assert.deepEqual(fileIdentity(join(builtDirectory, file.path)), { sha256: file.sha256, bytes: file.bytes });
  const inputs = read(join(builtDirectory, "inputs-before.json"));
  assert.deepEqual(read(join(builtDirectory, "inputs-after.json")), inputs);
  for (const file of inputs.files) assert.deepEqual(fileIdentity(file.path), { sha256: file.sha256, bytes: file.bytes });
  const current = [];
  for (const name of ["source", "original", "project-dependencies", "npm-runtime", "workspace"]) {
    const before = read(join(builtDirectory, `${name}-before.json`));
    assert.deepEqual(read(join(builtDirectory, `${name}-after.json`)), before);
    const after = ["project-dependencies", "npm-runtime"].includes(name) ? dependencyTree(before.root) : snapshotInputs(before.root, name === "original" ? { exclude: [".git", "node_modules", "target", ".cache"] } : undefined);
    assert.deepEqual(after, before, `${name} differs from frozen source-build inputs`);
    current.push({ name, sha256: after.sha256, entries: after.files.length });
  }
  assert.deepEqual(validateBuild({ exitCode: built.build.status, invocations: built.build.invocations, compilerSha256: builtReceipt.parent.compiler.sha256, contractSha256: built.contractSha256, verifyOutputs: true }), []);
  assert.equal(built.build.invocations.length, 2);
  const report = read(join(builtDirectory, "tests/mdast-util-to-hastlil/report.json"));
  const inventory = read(join(builtDirectory, "required-tests.json"));
  assert.deepEqual(report.evidence.cases.map(row => row.id).sort(), inventory.requiredCases);
  assert.equal(new Set(report.evidence.cases.map(row => row.id)).size, 152);
  assert.deepEqual(report.errors, validateTests(report.evidence));
  assert(report.errors.length > 0);
  assert(report.errors.every(error => error === "test command failed" || error.startsWith("test did not pass: ") || error.startsWith("required case did not pass: ")));
  assert(report.prerequisites.every(row => row.attempted && row.status === 0 && !row.error && !row.signal));
  for (const loaded of report.evidence.loadedArtifacts) {
    const artifact = built.artifacts.find(row => basename(row.path) === basename(loaded.path));
    assert(artifact, `unexpected loaded artifact: ${loaded.path}`);
    assert.equal(loaded.sha256, artifact.sha256);
    assert.equal(loaded.bytes, artifact.raw);
    assert.deepEqual(fileIdentity(join(builtDirectory, "artifacts", basename(loaded.path))), { sha256: loaded.sha256, bytes: loaded.bytes });
  }
  assert.deepEqual([...new Set(report.evidence.loadedArtifacts.map(row => basename(row.path)))].sort(), ["to-hast.closed.js", "to-hast.esm.js"]);
  receipt.frozenReceipt = identity(builtReceiptPath);
  receipt.verifiedOutputs = builtReceipt.outputs.length;
  receipt.verifiedInputPins = inputs.files.length;
  receipt.currentClosures = current;
  receipt.caseCounts = report.evidence.cases.reduce((counts, row) => { const key = `${row.type}:${row.status}`; counts[key] = (counts[key] ?? 0) + 1; return counts; }, {});
  receipt.loadedArtifacts = report.evidence.loadedArtifacts;
  const codec = builtReceipt.parent.codec.path;
  const paths = built.artifacts.map(row => join(builtDirectory, "artifacts", basename(row.path)));
  pins = [fileURLToPath(import.meta.url), process.execPath, codec, builtReceiptPath, resolve(here, "../../../finer/tools/artifact-evidence.mjs"), resolve(here, "../../../finer/tools/bounded-command.mjs"), ...paths].map(identity);
  assert.deepEqual(fileIdentity(codec), { sha256: builtReceipt.parent.codec.sha256, bytes: builtReceipt.parent.codec.bytes });
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity("/home/azureuser/.nvm/versions/node/v24.11.1/bin/node"));
  save("inputs-before.json", pins);
  copyFileSync(fileURLToPath(import.meta.url), join(output, "verification-source.mjs"));
  save("receipt.json", receipt);
  const result = await runBoundedCommand(codec, ["--json", ...paths], { cwd: here, timeoutMs: 60_000, maxBuffer: 64 * 1024 * 1024, encoding: "utf8" });
  writeFileSync(join(output, "codec.stdout"), result.stdout);
  writeFileSync(join(output, "codec.stderr"), result.stderr);
  receipt.command = { executable: codec, args: ["--json", ...paths], status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision };
  assert.equal(result.status, 0);
  assert(!result.signal && !result.error && !result.supervision.timedOut);
  const scores = validateMeasurements(JSON.parse(result.stdout), paths);
  for (const artifact of built.artifacts) for (const key of ["raw", "gzip9", "brotli11"]) assert.equal(scores.get(join(builtDirectory, "artifacts", basename(artifact.path)))[key], artifact[key]);
  receipt.artifacts = built.artifacts;
  receipt.codecStdout = identity(join(output, "codec.stdout"));
  receipt.passed = true;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    if (pins) {
      const after = pins.map(row => identity(row.path));
      save("inputs-after.json", after);
      assert.deepEqual(after, pins);
      receipt.inputsStable = true;
    }
  } catch (error) { receipt.passed = false; receipt.inputsStable = false; receipt.validationFailure = error.message; process.exitCode = 1; }
  receipt.completed = new Date().toISOString();
  save("receipt.json", receipt);
  console.log(JSON.stringify({ passed: receipt.passed, originalTestBoundaryPassed: false, verifiedOutputs: receipt.verifiedOutputs, caseCounts: receipt.caseCounts, failure: receipt.failure?.message }));
}
