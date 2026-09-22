import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { loadavg, tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs, validateBuild, validateTests } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
assert.equal(process.argv.length, 6, "usage: qualify-marked-release.mjs accepted-receipt.json compiler codec new-output-directory");
const [parentPath, compiler, codec, output] = process.argv.slice(2).map(path => resolve(path));
mkdirSync(output, { recursive: false });
const parentDirectory = dirname(parentPath);
const original = "/home/azureuser/markedlil";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const runner = join(root, "finer/tools/portgate.mjs");
const workloadPath = join(root, "benchmarks/libraries/maintained-workloads.json");
const inventoryPath = join(root, "benchmarks/libraries/markedlil.profiles.tests.json");
const fixturePath = join(root, "finer/tools/port-adapters/marked-profiles.test.mjs");
const identify = path => ({ path, ...fileIdentity(path) });
const json = path => JSON.parse(readFileSync(path, "utf8"));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const profiles = [
  { config: "lilscript.toml", output: "dist/marked.raw.js", objective: "brotli" },
  { config: "lilscript.closed.toml", output: "dist/marked.closed.js", objective: "brotli" },
  { config: "lilscript.gzip.toml", output: "dist/marked.gzip.js", objective: "gzip" },
  { config: "lilscript.bytes.toml", output: "dist/marked.bytes.js", objective: "raw" },
];
const evidencePaths = [
  fileURLToPath(import.meta.url), node, compiler, codec, parentPath,
  ...["inputs-before.json", "inputs-after.json"].map(name => join(parentDirectory, name)),
  join(root, ".nvmrc"), workloadPath, inventoryPath, fixturePath,
  ...["portgate", "artifact-evidence", "compiler-receipt", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts", "vitest-test-evidence"].map(name => join(root, `finer/tools/${name}.mjs`)),
];
function pins() {
  const files = evidencePaths.map(identify);
  return { files, sha256: fingerprint(files) };
}
function compilerInputs() {
  const files = [];
  function walk(path) {
    if (!existsSync(path)) return;
    if (statSync(path).isDirectory()) for (const name of readdirSync(path).sort()) walk(join(path, name));
    else files.push({ path: relative(root, path), ...fileIdentity(path) });
  }
  for (const path of [".cargo", ".nvmrc", "Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml", "src", "tests", "examples", "vendor", "finer/tools/semantic-integration.py", "finer/tools/bounded-command.mjs", "benchmarks/migration-results/2026-09-19-artifact-service/qualify.mjs"]) walk(join(root, path));
  files.sort((a, b) => a.path.localeCompare(b.path));
  return { files, sha256: hash(JSON.stringify(files)) };
}
// Include nested dependency directories and symlink destinations, unlike a source snapshot.
function dependencies() {
  const directory = join(original, "node_modules");
  const files = [];
  let bytes = 0;
  function walk(path, ancestors = new Set()) {
    const actual = realpathSync(path);
    const info = statSync(path);
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
const receipt = {
  schema: 1, kind: "source-pinned-release-marked-baseline", started: new Date().toISOString(),
  passed: false, maintainedBoundaryPassed: false, qualification: "unverified", commands: [], profiles,
  timeoutMs: { build: 300_000, tests: 300_000, codec: 300_000, supervisor: 930_000 },
  scope: "one unchanged maintained Marked parse-only boundary, all four source-build profiles and the original 75-node production inventory",
  limitations: [
    "This runs the library's unchanged legacy compiler backend, not whole-library support in the semantic backend.",
    "The 75 suite/test nodes include the original five package test files and shared three-objective cases; no case, profile or existing exclusion is changed.",
    "Known fresh raw-profile failures remain failures; no retained incumbent or upstream artifact may substitute for a fresh output.",
    "Additional declared closed-profile, exact UMD/browser and installed-package coverage remains unverified even if this maintained boundary passes.",
    "D2 public-observation policy and D5 startup/profile decisions remain open; the unchanged profiles use level 15, not a new level-16 permission.",
    "Timeouts are supervision deadlines with bounded termination grace, not hard OS resource limits; escaped process sessions are outside process-group cleanup.",
    "No network installation, compiler build, library edit or release-performance comparison occurs here. Host load is recorded but neither controlled nor isolated.",
    "Original sources and the existing complete dependency tree are hashed, not claimed to be a hermetic machine or system-library environment.",
  ],
  loadAverageStart: loadavg(),
};
const overrides = {
  PATH: "/home/azureuser/.nvm/versions/node/v24.11.1/bin:/usr/local/bin:/usr/bin:/bin",
  NODE_OPTIONS: "", NODE_PATH: "", RAYON_NUM_THREADS: "2",
  npm_config_offline: "true", npm_config_ignore_scripts: "true", npm_config_audit: "false", npm_config_fund: "false",
};
const env = { ...process.env, ...overrides };
delete env.NODE_TEST_CONTEXT;
receipt.environment = { overrides, otherwise: "inherited", removed: ["NODE_TEST_CONTEXT"] };
let initialPins, beforeSource, beforeCompleteSource, beforeDependencies, beforeCompiler;
let arm;
save("receipt.json", receipt);
try {
  initialPins = pins();
  save("inputs-before.json", initialPins);
  copyFileSync(fileURLToPath(import.meta.url), join(output, "wrapper.mjs"));
  const toolsDirectory = join(output, "runner-sources");
  mkdirSync(toolsDirectory);
  for (const path of evidencePaths.filter(path => path.startsWith(join(root, "finer/tools/")))) {
    const destination = join(toolsDirectory, relative(join(root, "finer/tools"), path));
    mkdirSync(dirname(destination), { recursive: true });
    copyFileSync(path, destination);
  }
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.equal(fileIdentity(process.execPath).sha256, fileIdentity(node).sha256);
  const parent = json(parentPath);
  assert.equal(parent.passed, true, "parent qualification did not pass");
  assert.equal(parent.inputsStable, true);
  assert.equal(parent.profile, "release");
  assert.equal(parent.publicIntegration, true);
  for (const name of ["inputs-before.json", "inputs-after.json"]) {
    const snapshot = json(join(parentDirectory, name));
    assert.equal(hash(JSON.stringify(snapshot.files)), snapshot.sha256);
    assert.equal(snapshot.sha256, parent.inputSha256);
    copyFileSync(join(parentDirectory, name), join(output, `parent-${name}`));
  }
  copyFileSync(parentPath, join(output, "parent-receipt.json"));
  for (const [name, path] of [["lilscript", compiler], ["lilscript-codec", codec]]) {
    const profile = parent.deliveryBuildProfiles.find(row => row.target === name);
    assert(profile, `missing parent build profile: ${name}`);
    assert.equal(profile.profile.opt_level, "3");
    assert.equal(profile.profile.debug_assertions, false);
    assert.equal(profile.profile.overflow_checks, false);
    const built = parent.binaries.find(row => row.path === profile.path);
    assert(built, `missing parent executable identity: ${name}`);
    assert.deepEqual(fileIdentity(path), { sha256: built.sha256, bytes: built.bytes });
  }
  beforeCompiler = compilerInputs();
  assert.equal(beforeCompiler.sha256, parent.inputSha256, "current compiler inputs differ from accepted release parent");
  save("compiler-inputs-before.json", beforeCompiler);
  receipt.parent = { ...identify(parentPath), inputSha256: parent.inputSha256, compiler: identify(compiler), codec: identify(codec) };
  const workload = json(workloadPath).libraries.find(row => row.id === "markedlil");
  assert.equal(workload.workspace, original);
  assert.equal(workload.tests.nodeAdapter.caseInventory, "markedlil.profiles.tests.json");
  assert.equal(workload.tests.nodeAdapter.sharedFixture, "marked-profiles.test.mjs");
  const inventory = json(inventoryPath);
  assert.equal(inventory.requiredCases.length, 75);
  assert.equal(new Set(inventory.requiredCases).size, 75);
  assert.equal(inventory.testFiles.length, 6);
  for (const file of inventory.testFiles) assert.equal(fileIdentity(file.path === ".lilscript-test-adapter/suite.test.mjs" ? fixturePath : join(original, file.path)).sha256, file.sha256);
  for (const file of inventory.fixtures) assert.equal(fileIdentity(join(original, file.path)).sha256, file.sha256);
  receipt.additionalRequiredCoverage = inventory.additionalRequiredCoverage;
  beforeSource = snapshotInputs(original);
  beforeCompleteSource = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
  beforeDependencies = dependencies();
  save("source-before.json", beforeSource);
  save("original-before.json", beforeCompleteSource);
  save("dependencies-before.json", beforeDependencies);
  for (const [path, name] of [[workloadPath, "maintained-workloads.json"], [inventoryPath, "markedlil.profiles.tests.json"], [fixturePath, "marked-profiles.test.mjs"]]) copyFileSync(path, join(output, name));
  arm = join(mkdtempSync(join(tmpdir(), "lilscript-marked-release-20260919-")), "portgate");
  receipt.arm = arm;
  const args = [runner, "record", "--arm", "source-pinned-release-marked", "--compiler", compiler, "--codec", codec, "--ports", "markedlil", "--manifest", workloadPath, "--out", arm, "--timeout", "300"];
  save("receipt.json", receipt);
  const started = process.hrtime.bigint();
  const result = await runBoundedCommand(node, args, { cwd: root, env, timeoutMs: receipt.timeoutMs.supervisor, maxBuffer: 64 * 1024 * 1024 });
  writeFileSync(join(output, "portgate.stdout"), result.stdout);
  writeFileSync(join(output, "portgate.stderr"), result.stderr);
  receipt.commands.push({ command: node, args, cwd: root, status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision, wall_ns: Number(process.hrtime.bigint() - started) });
  save("receipt.json", receipt);
  assert.equal(result.error, undefined, "portgate supervisor failed; partial evidence retained");
  assert.equal(result.signal, null);
  assert([0, 1].includes(result.status), "unexpected portgate exit status");
  const manifest = json(join(arm, "manifest.json"));
  const built = json(join(arm, "markedlil.json"));
  assert.deepEqual(manifest.ports, ["markedlil"]);
  assert.equal(manifest.compiler.sha256, fileIdentity(compiler).sha256);
  assert.equal(manifest.codec.sha256, fileIdentity(codec).sha256);
  assert.equal(manifest.workloadManifest.sha256, fileIdentity(workloadPath).sha256);
  assert.deepEqual(built.source.snapshot.files, beforeSource.files, "runner source differs from pinned original");
  receipt.build = built.build;
  receipt.artifacts = built.artifacts;
  receipt.tests = built.tests;
  assert.deepEqual(validateBuild({ exitCode: built.build.status, invocations: built.build.invocations, compilerSha256: manifest.compiler.sha256, contractSha256: built.contractSha256, verifyOutputs: true }), []);
  assert.equal(built.build.trust, "ok");
  assert.equal(built.build.supervision.timeoutMs, receipt.timeoutMs.build);
  assert.equal(built.measurement.supervision.timeoutMs, receipt.timeoutMs.codec);
  assert.equal(built.build.invocations.length, profiles.length);
  for (const profile of profiles) {
    const invocation = built.build.invocations.find(row => relative(built.workspace, row.output.path) === profile.output);
    assert(invocation, `missing fresh profile: ${profile.config}`);
    assert.deepEqual(invocation.args, [join(built.workspace, "src/entry.lil"), "--target", "js-module", "--config", join(built.workspace, profile.config), "-o", join(built.workspace, profile.output)]);
    assert.equal(invocation.configuration.sha256, fileIdentity(join(original, profile.config)).sha256);
    for (const file of beforeSource.files) assert.deepEqual(invocation.inputs.find(row => row.path === file.path), file, `compiler input changed: ${file.path}`);
  }
  for (const artifact of built.artifacts) {
    assert.deepEqual(fileIdentity(join(built.workspace, artifact.path)), { sha256: artifact.sha256, bytes: artifact.raw });
    assert([artifact.raw, artifact.gzip9, artifact.brotli11].every(value => Number.isSafeInteger(value) && value >= 0));
  }
  const report = json(built.tests.report);
  assert.equal(report.supervision.timeoutMs, receipt.timeoutMs.tests);
  assert.deepEqual(report.evidence.requiredCases, inventory.requiredCases);
  assert.equal(report.evidence.cases.length, 75);
  assert.deepEqual(validateTests(report.evidence), []);
  assert.deepEqual(report.errors, []);
  assert.equal(report.certification, "verified");
  const outstanding = inventory.additionalRequiredCoverage.map(coverage => `additional required coverage remains unverified: ${typeof coverage === "string" ? coverage : JSON.stringify(coverage)}`);
  assert.deepEqual(built.tests.errors, outstanding);
  assert.equal(result.status, outstanding.length ? 1 : 0);
  receipt.maintainedBoundaryPassed = true;
  receipt.passed = true;
  receipt.outcome = "Fresh four-profile build and all 75 maintained nodes passed; additional declared coverage remains open.";
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const finalPins = pins();
    save("inputs-after.json", finalPins);
    receipt.inputsStable = initialPins?.sha256 === finalPins.sha256;
    assert.equal(receipt.inputsStable, true, "evidence inputs changed");
    for (const [name, before, after] of [
      ["source", beforeSource, beforeSource && snapshotInputs(original)],
      ["original", beforeCompleteSource, beforeCompleteSource && snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] })],
      ["dependencies", beforeDependencies, beforeDependencies && dependencies()],
      ["compiler-inputs", beforeCompiler, beforeCompiler && compilerInputs()],
    ]) if (before) {
      save(`${name}-after.json`, after);
      assert.equal(after.sha256, before.sha256, `${name} changed`);
    }
  } catch (error) {
    receipt.passed = receipt.maintainedBoundaryPassed = false;
    receipt.validationFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  try {
    if (arm && existsSync(arm)) {
      for (const name of ["manifest.json", "markedlil.json", "logs", "wrappers", "invocations", "tests"]) if (existsSync(join(arm, name))) cpSync(join(arm, name), join(output, name), { recursive: true });
      const dist = join(arm, "workspaces/markedlil/dist");
      if (existsSync(dist)) cpSync(dist, join(output, "artifacts"), { recursive: true });
    }
  } catch (error) {
    receipt.passed = receipt.maintainedBoundaryPassed = false;
    receipt.archiveFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.loadAverageEnd = loadavg();
  receipt.completed = new Date().toISOString();
  try {
    receipt.outputs = readdirSync(output).some(name => name !== "receipt.json")
      ? snapshotInputs(output, { exclude: ["receipt.json"] }).files : [];
  } catch (error) {
    receipt.passed = receipt.maintainedBoundaryPassed = false;
    receipt.outputSnapshotFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  save("receipt.json", receipt);
  console.log(JSON.stringify({ output, passed: receipt.passed, maintainedBoundaryPassed: receipt.maintainedBoundaryPassed, qualification: receipt.qualification, failure: receipt.failure?.message, inputsStable: receipt.inputsStable }));
}
