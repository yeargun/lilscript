import assert from "node:assert/strict";
import { copyFileSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
const original = "/home/azureuser/jquerylil", lab = join(root, "benchmarks/popular");
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const script = join(lab, "verify-jquery.mjs"), helper = join(lab, "jquery-benchmark-artifact.mjs");
const candidate = join(original, "dist/jquery.esm.js");
const candidateSha256 = "b43df6f9adc4bf1b69d523b6f7a98ff4b9348b481606a4946e5b9ba0e28de0e0";
const parentDirectory = join(here, "existing-dist"), parentPath = join(parentDirectory, "receipt.json");
const parentSha256 = "c622329fefc6428424d3b3fb342a29dceb875b8ecd360877040e6eeb96191ddb";
const output = join(here, "consumer-supplement");
mkdirSync(output, { recursive: false });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const identify = path => ({ path, ...fileIdentity(path) });
const json = path => JSON.parse(readFileSync(path, "utf8"));
const toolPaths = ["artifact-evidence", "bounded-command", "observe-node-artifacts"].map(name => join(root, `finer/tools/${name}.mjs`));
const observer = toolPaths[2];
const pinPaths = [fileURLToPath(import.meta.url), node, join(root, ".nvmrc"), ...toolPaths, script, helper, join(lab, "package.json"), join(lab, "package-lock.json"), parentPath];
const pins = () => { const files = pinPaths.map(identify); return { files, sha256: fingerprint(files) }; };

// Match discovery's complete-tree identities, including nested symlink targets.
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

function buildFiles() {
  const directory = join(lab, "build");
  return existsSync(directory) && readdirSync(directory).length ? snapshotInputs(directory).files : [];
}
function snapshot() {
  return {
    inputs: pins(), source: snapshotInputs(original),
    original: snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] }),
    project: dependencyTree(join(original, "node_modules")),
    labDependencies: dependencyTree(join(lab, "node_modules")), labBuildFiles: buildFiles(),
  };
}
const removed = Object.keys(process.env).filter(key => key.startsWith("LILSCRIPT_") || key.startsWith("JQUERY_LILSCRIPT_") || /^npm_/i.test(key) || ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
const environment = { ...process.env };
for (const key of removed) delete environment[key];
const overrides = {
  PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "",
  JQUERY_LILSCRIPT_ARTIFACT: candidate, JQUERY_LILSCRIPT_ARTIFACT_SHA256: candidateSha256,
};
Object.assign(environment, overrides);
const markers = ["core", "deferred", "data", "queue", "attributes"].map(name => `jquery-upstream:${name}:ok`);
markers.push(`jquery-upstream:events:ok artifact=${candidateSha256}`);
const started = process.hrtime.bigint();
const elapsedMs = () => Number(process.hrtime.bigint() - started) / 1e6;
const receipt = {
  schema: 1, kind: "jquery-retained-artifact-original-consumer-supplement", started: new Date().toISOString(),
  passed: false, qualification: "unverified", sourceBuilt: false,
  boundsMs: { externalSupervisor: 90_000, sharedAttempt: 90_000, killGrace: 1000 }, maxBufferBytes: 64 * 1024 * 1024,
  parent: { path: parentPath, expectedSha256: parentSha256 }, originalCommand: "node benchmarks/popular/verify-jquery.mjs",
  requiredCompletionMarkers: markers, environment: { removed, overrides, otherwise: "inherited" },
  limitations: [
    "One unchanged top-level assertion script, not Node test cases and not additions to the frozen eight compatibility identities.",
    "Existing retained ESM only; no compiler, esbuild build, npm, install, codec, CJS, UMD, package or source-built qualification.",
    "The existing artifact override skips the script's compiler/esbuild branch. Its unconditional mkdir may create an empty popular/build directory; existing files must remain unchanged.",
    "Popular's installed jquery@3.7.1 is the separate upstream oracle; its dependency tree differs from jquerylil's and both complete trees are pinned.",
    "The original JSDOM assertions cover core, Deferred, data, queue, attributes/CSS and events, not full upstream QUnit/browser or the separate port's other consumer suites.",
    "Full source/dependency and executed-tool identities are recorded; the operating system and inherited host environment are not hermetic. No npm command executes, so global npm is not an input.",
    "The outer caller must actually apply the declared 90-second process-group bound. Shared-host elapsed time is diagnostic only."
  ],
};
let before;
save("receipt.json", receipt);
try {
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  const upstream = createRequire(script).resolve("jquery");
  assert.equal(fileIdentity(parentPath).sha256, parentSha256);
  const parent = json(parentPath);
  assert.equal(parent.passed, true);
  assert.equal(parent.inputsStable, true);
  assert.equal(parent.sourceBuilt, false);
  assert.deepEqual(snapshotInputs(parentDirectory, { exclude: ["receipt.json"] }).files, parent.outputs);
  const retained = json(join(parentDirectory, "before.json"));
  assert.deepEqual(json(join(parentDirectory, "after.json")), retained);
  before = snapshot();
  save("before.json", before);
  for (const key of ["source", "original", "project"]) assert.deepEqual(before[key], retained[key], `jquery discovery ${key} changed`);
  const expectedArtifact = retained.original.files.find(row => row.path === "dist/jquery.esm.js");
  assert.equal(expectedArtifact.sha256, candidateSha256);
  assert.deepEqual(fileIdentity(candidate), { sha256: candidateSha256, bytes: expectedArtifact.bytes });
  receipt.candidate = identify(candidate);
  receipt.oracle = identify(upstream);
  mkdirSync(join(output, "parent"));
  copyFileSync(parentPath, join(output, "parent/receipt.json"));
  for (const row of parent.outputs) {
    const destination = join(output, "parent", row.path);
    mkdirSync(dirname(destination), { recursive: true });
    copyFileSync(join(parentDirectory, row.path), destination);
  }
  copyFileSync(candidate, join(output, "jquery.esm.mjs"));
  assert.deepEqual(fileIdentity(join(output, "jquery.esm.mjs")), fileIdentity(candidate));
  copyFileSync(fileURLToPath(import.meta.url), join(output, "supplement-source.mjs"));
  mkdirSync(join(output, "runner-sources"));
  for (const path of [...toolPaths, script, helper, join(lab, "package.json"), join(lab, "package-lock.json")]) copyFileSync(path, join(output, "runner-sources", basename(path)));
  const loadedDirectory = join(output, "loaded"), preload = join(output, "observe.mjs");
  const watched = [candidate, script, helper, upstream];
  writeFileSync(preload, `import { observeNodeArtifacts } from ${JSON.stringify(pathToFileURL(observer).href)};\nobserveNodeArtifacts(${JSON.stringify({ paths: watched, directory: loadedDirectory })});\n`);
  const timeoutMs = Math.floor(90_000 - elapsedMs());
  assert(timeoutMs > 0, "supplement budget exhausted before original consumer");
  const args = ["--import", preload, script];
  receipt.command = { command: node, args, cwd: lab, timeoutMs };
  save("receipt.json", receipt);
  const result = await runBoundedCommand(node, args, { cwd: lab, env: environment, timeoutMs, killAfterMs: 1000, maxBuffer: receipt.maxBufferBytes, encoding: "utf8" });
  writeFileSync(join(output, "consumer.stdout"), result.stdout);
  writeFileSync(join(output, "consumer.stderr"), result.stderr);
  receipt.result = { pid: result.pid, status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision };
  save("receipt.json", receipt);
  const loads = existsSync(loadedDirectory) ? readdirSync(loadedDirectory).sort().map(name => json(join(loadedDirectory, name))) : [];
  receipt.observedLoads = loads;
  receipt.completionMarkers = result.stdout.split(/\r?\n/).filter(line => markers.includes(line));
  assert.equal(result.status, 0, "original consumer exited unsuccessfully");
  assert.equal(result.signal, null);
  assert.equal(result.error, undefined);
  assert.equal(result.supervision.timedOut, false);
  assert.deepEqual(receipt.completionMarkers, markers, "missing, duplicated or reordered original completion markers");
  for (const path of watched) {
    const matching = loads.filter(row => row.kind === "node-module-load" && row.path === path);
    assert(matching.length > 0, `missing exact Node load: ${path}`);
    for (const row of matching) {
      assert.equal(row.pid, result.pid);
      assert.deepEqual({ sha256: row.sha256, bytes: row.bytes }, fileIdentity(path));
    }
  }
  assert.deepEqual(fileIdentity(join(output, "jquery.esm.mjs")), fileIdentity(candidate));
  receipt.passed = true;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const after = snapshot();
    save("after.json", after);
    assert.deepEqual(after, before, "supplement inputs changed");
    const parent = json(parentPath);
    assert.equal(fileIdentity(parentPath).sha256, parentSha256);
    assert.deepEqual(snapshotInputs(parentDirectory, { exclude: ["receipt.json"] }).files, parent.outputs);
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
  console.log(JSON.stringify({ output, passed: receipt.passed, inputsStable: receipt.inputsStable, markers: receipt.completionMarkers?.length, failure: receipt.failure?.message }));
}
