import assert from "node:assert/strict";
import { existsSync, copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs } from "../../../finer/tools/artifact-evidence.mjs";
import { runNodeTestEvidence } from "../../../finer/tools/node-test-evidence.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
const original = "/home/azureuser/micromarklil";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const output = join(here, "existing-dist");
mkdirSync(output, { recursive: false });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const identify = path => ({ path, ...fileIdentity(path) });
const local = path => ({ path, ...fileIdentity(join(original, path)) });
const removed = Object.keys(process.env).filter(key => key.startsWith("LILSCRIPT_") || /^npm_/i.test(key) || ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
for (const key of removed) delete process.env[key];
const overrides = {
  PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "",
};
Object.assign(process.env, overrides);
const toolPaths = ["artifact-evidence", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts"].map(name => join(root, `finer/tools/${name}.mjs`));
const pinPaths = [fileURLToPath(import.meta.url), node, join(root, ".nvmrc"), ...toolPaths];
const pins = () => { const files = pinPaths.map(identify); return { files, sha256: fingerprint(files) }; };

// Same bounded complete-tree capture used by the existing discovery receipts.
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

const files = ["test/official/index.js", "test/public-api.test.mjs", "test/stream-api.test.mjs"];
const fixtures = ["package.json", "package-lock.json", "scripts/build.mjs", "lilscript.toml", "lilscript.closed.toml"].map(local);
const started = process.hrtime.bigint();
const elapsedMs = () => Number(process.hrtime.bigint() - started) / 1e6;
const receipt = {
  schema: 1, kind: "existing-dist-micromark-inventory-discovery", started: new Date().toISOString(),
  passed: false, qualification: "unverified", sourceBuilt: false,
  boundsMs: { externalSupervisor: 90_000, sharedAttempt: 90_000, nodeSuiteCeiling: 90_000 },
  originalCommand: `node --test ${files.join(" ")}`,
  environment: { removed, overrides, otherwise: "inherited; no npm command or package installation executes" },
  testOnlyArtifact: "dist/micromark.test.js",
  limitations: [
    "Existing distribution only; no compiler execution, source-built qualification, codec scoring or new assertions.",
    "Original npm test runs unchanged. Seven observed files include six public root/stream/closed/UMD deliveries and one test-only utility artifact; utility checks do not qualify public production delivery.",
    "UMD executes by import in Node, not a real browser. Installed-package, declaration, check:pack and check:site coverage remain outside the selected original npm test.",
    "No prerequisite is declared by original npm test. No npm command, compiler, dependency installation or optional typecheck executes.",
    "Complete source and installed project dependencies including symlink destinations, Node and observer tools are pinned; OS libraries and the whole host environment are not hermetic.",
    "Original stream tests create and remove four temporary files in the workspace; those paths must be absent initially and the final complete tree must match.",
    "Passing current numeric Proxy assertions does not settle arbitrary truthy initial-point coercion or the open D2 public-JS policy."
  ],
};
let before;
save("receipt.json", receipt);
try {
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  assert.equal(JSON.parse(readFileSync(join(original, "package.json"), "utf8")).scripts.test, receipt.originalCommand);
  for (const name of ["integrate-input", "integrate-output", "non-utf8-input", "non-utf8-output"]) {
    assert(!existsSync(join(original, name)), `original test scratch already exists: ${name}`);
  }
  before = {
    inputs: pins(), source: snapshotInputs(original),
    original: snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] }),
    project: dependencyTree(join(original, "node_modules")),
  };
  save("before.json", before);
  save("declarations.json", { files, fixtures, prerequisites: [] });
  copyFileSync(fileURLToPath(import.meta.url), join(output, "discovery-source.mjs"));
  mkdirSync(join(output, "runner-sources"));
  for (const path of toolPaths) copyFileSync(path, join(output, "runner-sources", basename(path)));
  const timeoutMs = Math.floor(90_000 - elapsedMs());
  assert(timeoutMs > 0, "discovery budget exhausted before Node owner");
  const report = await runNodeTestEvidence({
    cwd: original, files, requiredFilePatterns: files,
    artifactPaths: ["dist/micromark.esm.js", "dist/micromark.cjs", "dist/micromark.closed.js", "dist/micromark.stream.js", "dist/micromark.stream.cjs", "dist/micromark.umd.js", "dist/micromark.test.js"],
    requiredCases: [], requiredTestFiles: files.map(local), requiredFixtures: fixtures,
    directory: join(output, "node"), timeoutMs,
  });
  receipt.report = identify(join(output, "node/report.json"));
  receipt.observedCases = report.evidence.cases;
  receipt.discoveryErrors = report.errors;
  receipt.passed = report.evidence.exitCode === 0 && report.evidence.cases.length > 0 && report.evidence.cases.every(row => row.status === "pass") && report.errors.length === 1 && report.errors[0] === "required test inventory missing or ambiguous";
  if (!receipt.passed) process.exitCode = 1;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const after = {
      inputs: pins(), source: snapshotInputs(original),
      original: snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] }),
      project: dependencyTree(join(original, "node_modules")),
    };
    save("after.json", after);
    assert.deepEqual(after, before, "discovery inputs changed");
    receipt.inputsStable = true;
  } catch (error) {
    receipt.passed = false;
    receipt.inputsStable = false;
    receipt.validationFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.elapsedMs = elapsedMs();
  receipt.completed = new Date().toISOString();
  try { receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files; }
  catch (error) { receipt.passed = false; receipt.outputManifestFailure = error.message; process.exitCode = 1; }
  save("receipt.json", receipt);
  console.log(JSON.stringify({ output, passed: receipt.passed, inputsStable: receipt.inputsStable, cases: receipt.observedCases?.length, failure: receipt.failure?.message }));
}
