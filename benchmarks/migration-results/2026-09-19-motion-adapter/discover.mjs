import assert from "node:assert/strict";
import { cpSync, existsSync, mkdtempSync, symlinkSync, copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs } from "../../../finer/tools/artifact-evidence.mjs";
import { runNodeTestEvidence } from "../../../finer/tools/node-test-evidence.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
const original = "/tmp/motionlil-cost-audit-20260911/current";
const dependencies = "/home/azureuser/motionlil/node_modules";
let workspace;
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const output = join(here, "existing-dist");
mkdirSync(output, { recursive: false });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const identify = path => ({ path, ...fileIdentity(path) });
const local = path => ({ path, ...fileIdentity(join(original, path)) });
const removed = Object.keys(process.env).filter(key => (key.startsWith("LILSCRIPT_") || key.startsWith("MOTIONLIL_")) || /^npm_/i.test(key) || ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
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

const files = ["test/node.test.mjs", "test/terser.test.mjs", "test/vite.test.mjs", "test/shake.test.mjs"];
const fixtures = ["package.json", "package-lock.json", "scripts/build.mjs", "src/lilscript.toml", "test/fixtures/vite/index.html", "test/fixtures/vite/main.js"].map(local);
const started = process.hrtime.bigint();
const elapsedMs = () => Number(process.hrtime.bigint() - started) / 1e6;
const receipt = {
  schema: 1, kind: "recovered-dependency-motion-node-inventory-discovery", started: new Date().toISOString(),
  passed: false, qualification: "unverified", sourceBuilt: false,
  boundsMs: { externalSupervisor: 90_000, sharedAttempt: 90_000, nodeSuiteCeiling: 90_000 },
  originalCommand: `node --test ${files.join(" ")}`,
  original, dependencies,
  environment: { removed, overrides, otherwise: "inherited; no npm command or package installation executes" },
  limitations: [
    "Recorded maintained source/dist is copied byte-for-byte to an isolated workspace; it is not replaced by the sibling source tree.",
    "Original recorded workspace has no resolvable installed dependencies. The pinned sibling dependency tree supplies matching locked direct versions for the unchanged npm-test selection; this is an explicit recovered environment, not the original installed closure.",
    "Playwright is absent from the recovered dependency tree. Original browser tests, declaration check, pack and site commands remain separate unexecuted obligations.",
    "Original npm test includes Terser, esbuild and Vite consumers. Their transformed outputs are not independently scored production artifacts; imported original distribution files have passive exact-byte evidence.",
    "No LilScript compiler, package installation, source/assertion/configuration edit, canonical codec scoring or current semantic-backend qualification occurs.",
    "Node dependency closure is pinned including symlink destinations. OS libraries and the entire host environment are not hermetic.",
    "Passing export coverage, prototype, arity, accessor and callback tests is not full Motion geometry or browser behavior coverage and does not settle D2."
  ]
};
let before;
save("receipt.json", receipt);
try {
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  const packageJson = JSON.parse(readFileSync(join(original, "package.json"), "utf8"));
  const lock = JSON.parse(readFileSync(join(original, "package-lock.json"), "utf8"));
  assert.equal(packageJson.scripts.test, receipt.originalCommand);
  assert(!existsSync(join(original, "node_modules")), "recorded workspace dependency state changed");
  receipt.recoveredDirectVersions = ["esbuild", "motion", "terser", "typescript", "vite"].map(name => {
    const actual = JSON.parse(readFileSync(join(dependencies, name, "package.json"), "utf8")).version;
    assert.equal(actual, packageJson.devDependencies[name], name);
    assert.equal(actual, lock.packages[`node_modules/${name}`].version, name);
    return { name, version: actual };
  });
  for (const name of ["test-output/terser-animate.mjs", "test-output/vite"]) {
    assert(!existsSync(join(original, name)), `original test scratch already exists: ${name}`);
  }
  workspace = join(mkdtempSync("/tmp/lilscript-motion-node-discovery-20260919-"), "motionlil");
  receipt.workspace = workspace;
  cpSync(original, workspace, {
    recursive: true,
    filter: path => ![".git", "node_modules", "target", ".cache"].includes(relative(original, path).split("/")[0]),
  });
  symlinkSync(dependencies, join(workspace, "node_modules"), "dir");
  const full = directory => snapshotInputs(directory, { exclude: [".git", "node_modules", "target", ".cache"] });
  before = {
    inputs: pins(), source: snapshotInputs(original), original: full(original),
    project: dependencyTree(dependencies), workspace: full(workspace),
  };
  assert.deepEqual(before.workspace.files, before.original.files, "isolated copy must preserve every original file");
  save("before.json", before);
  save("declarations.json", { files, fixtures, prerequisites: [] });
  copyFileSync(fileURLToPath(import.meta.url), join(output, "discovery-source.mjs"));
  mkdirSync(join(output, "runner-sources"));
  for (const path of toolPaths) copyFileSync(path, join(output, "runner-sources", basename(path)));
  const timeoutMs = Math.floor(90_000 - elapsedMs());
  assert(timeoutMs > 0, "discovery budget exhausted before Node owner");
  const report = await runNodeTestEvidence({
    cwd: workspace, files, requiredFilePatterns: files,
    artifactPaths: ["index.js", "index.cjs", "animate.js", "animate-mini.js", "scroll.js", "gestures.js", "viewport.js", "resize.js", "full.js", "mini.js", "debug.js"].map(name => `dist/${name}`),
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
      project: dependencyTree(dependencies),
      workspace: snapshotInputs(workspace, { exclude: [".git", "node_modules", "target", ".cache"] }),
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

