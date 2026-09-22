import assert from "node:assert/strict";
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { loadavg, tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { digest, fileIdentity, fingerprint, snapshotInputs, validateBuild, validateMeasurements, validateTests } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
assert.equal(process.argv.length, 3, "usage: qualify.mjs new-output-directory");
const output = resolve(process.argv[2]);
mkdirSync(output, { recursive: false });
const original = "/home/azureuser/remark-breakslil";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const npm = join(dirname(node), "npm"), npmRoot = dirname(dirname(realpathSync(npm)));
const preserved = "/tmp/lilscript-public-integration-release-baseline-20260919";
const compiler = join(preserved, "lilscript"), codec = join(preserved, "lilscript-codec");
const parentPath = join(root, "benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-19T16-36-23.430Z/receipt.json");
const workloadPath = join(root, "benchmarks/libraries/maintained-workloads.json");
const inventoryPath = join(root, "benchmarks/libraries/remark-breakslil.required-tests.json");
const toolPaths = ["portgate", "artifact-evidence", "compiler-receipt", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts", "vitest-test-evidence"].map(name => join(root, `finer/tools/${name}.mjs`));
const scratch = mkdtempSync(join(tmpdir(), "lilscript-remark-breaks-20260919-"));
const arm = join(scratch, "portgate"), workspace = join(arm, "workspaces/remark-breakslil");
const userConfig = join(output, "npm-user.npmrc"), globalConfig = join(output, "npm-global.npmrc");
writeFileSync(userConfig, "");
writeFileSync(globalConfig, "");
const identify = path => ({ path, ...fileIdentity(path) });
const json = path => JSON.parse(readFileSync(path, "utf8"));
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const evidencePaths = [fileURLToPath(import.meta.url), node, npm, compiler, codec, parentPath, ...["inputs-before.json", "inputs-after.json"].map(name => join(dirname(parentPath), name)), workloadPath, inventoryPath, join(root, ".nvmrc"), userConfig, globalConfig, ...toolPaths];
const pins = () => { const files = evidencePaths.map(identify); return { files, sha256: fingerprint(files) }; };

// Same bounded tree capture used by the Marked baseline, including nested npm dependencies.
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

const started = process.hrtime.bigint();
const elapsed = () => Number(process.hrtime.bigint() - started) / 1e6;
const remaining = ceiling => {
  const milliseconds = Math.floor(430_000 - elapsed());
  assert(milliseconds > 0, "shared 430-second command budget exhausted");
  return Math.min(ceiling, milliseconds);
};
const removed = Object.keys(process.env).filter(key => key.startsWith("LILSCRIPT_") || /^npm_/i.test(key) || ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
const overrides = {
  PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "", RAYON_NUM_THREADS: "2",
  npm_config_offline: "true", npm_config_ignore_scripts: "true", npm_config_audit: "false", npm_config_fund: "false",
  npm_config_userconfig: userConfig, npm_config_globalconfig: globalConfig, npm_config_cache: join(scratch, "npm-cache"),
};
const env = { ...process.env };
for (const key of removed) delete env[key];
Object.assign(env, overrides);
const receipt = {
  schema: 1, kind: "source-pinned-remark-breaks-baseline", started: new Date().toISOString(), arm, workspace,
  passed: false, originalTestBoundaryPassed: false, packageDryRunPassed: false, qualification: "unverified", commands: [],
  boundsMs: { build: 180_000, nodeSuite: 90_000, prerequisiteCeiling: 90_000, codec: 60_000, pack: 60_000, sharedCommands: 430_000, externalSupervisor: 450_000 },
  environment: { overrides, removed, otherwise: "inherited; original workspace .npmrc preserved if present" },
  scope: "Unchanged two-invocation source build, original type prerequisite and all 23 Node identities, followed by the separate original package dry-run check",
  limitations: [
    "Compiler source identity is the preserved accepted release parent, not current compiler sources or semantic-backend library support.",
    "The 23 original Node identities include 20 test nodes (including parent tests) and 3 suites; no extra Node case IDs are invented for command prerequisites.",
    "ESM/CJS package self-import and direct closed-module execution are observed. Original CJS assertions cover default-only export/function shape; behavior and arity assertions use ESM, so no full CJS behavior suite is claimed.",
    "No newly packed/installed consumer or exact UMD/browser runtime qualification is claimed.",
    "The original Node site test only reads existing files. Original check:site/build:site remains unexecuted; package dry-run success is not browser or installed-package evidence.",
    "All JavaScript files are scored independently, including unexecuted raw/UMD outputs; no competitive gain, combined-stream delivery optimum, or public-observation policy change follows.",
    "Source, installed project dependencies, global npm including nested dependencies, and explicit tools are pinned; OS libraries and the whole host environment are not hermetic.",
    "Per-phase and per-prerequisite caps are individual command bounds, not additive performance evidence. Outer supervision also bounds the shared attempt; escaped sessions and supervisor SIGKILL cleanup are not covered.",
    "No compiler build, network dependency installation, source edit, altered original assertion, or retry is performed. Host load is recorded but not isolated; no speed claim.",
  ], loadAverageStart: loadavg(),
};
let initialPins, sourceBefore, originalBefore, projectBefore, npmBefore, workspaceBefore;
save("receipt.json", receipt);

async function run(label, command, args, cwd, ceiling) {
  const begin = process.hrtime.bigint();
  const result = await runBoundedCommand(command, args, { cwd, env, timeoutMs: remaining(ceiling), maxBuffer: 64 * 1024 * 1024 });
  writeFileSync(join(output, `${label}.stdout`), result.stdout);
  writeFileSync(join(output, `${label}.stderr`), result.stderr);
  receipt.commands.push({ label, command, args, cwd, status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision, wall_ns: Number(process.hrtime.bigint() - begin) });
  save("receipt.json", receipt);
  return result;
}
const completed = result => !result.error && !result.signal && !result.supervision.timedOut;

try {
  initialPins = pins();
  save("inputs-before.json", initialPins);
  copyFileSync(fileURLToPath(import.meta.url), join(output, "wrapper.mjs"));
  mkdirSync(join(output, "runner-sources"));
  for (const path of toolPaths) copyFileSync(path, join(output, "runner-sources", basename(path)));
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  assert.equal(fileIdentity(parentPath).sha256, "020c288f5b0e6bafc016e12cbb6d5a7a7ab53b9b28a32444c584c9781abbf533");
  const parent = json(parentPath);
  assert(parent.passed && parent.inputsStable && parent.profile === "release" && parent.publicIntegration);
  assert.equal(parent.inputSha256, "2e140666dd1cb5fd2f0cf82d0256cc0ed15e630a1defaaf4eca549c887e1e2f3");
  copyFileSync(parentPath, join(output, "parent-receipt.json"));
  for (const name of ["inputs-before.json", "inputs-after.json"]) {
    const path = join(dirname(parentPath), name), input = json(path);
    assert.equal(digest(JSON.stringify(input.files)), parent.inputSha256);
    assert.equal(input.sha256, parent.inputSha256);
    copyFileSync(path, join(output, `parent-${name}`));
  }
  for (const path of [compiler, codec]) {
    const row = parent.binaries.find(row => basename(row.path) === basename(path));
    assert(row, `missing accepted binary: ${path}`);
    assert.deepEqual(fileIdentity(path), { sha256: row.sha256, bytes: row.bytes });
  }
  receipt.parent = { ...identify(parentPath), inputSha256: parent.inputSha256, compiler: identify(compiler), codec: identify(codec) };
  const inventory = json(inventoryPath), workload = json(workloadPath).libraries.find(row => row.id === "remark-breakslil");
  assert.equal(workload.workspace, original);
  assert.equal(workload.tests.nodeAdapter.caseInventory, "remark-breakslil.required-tests.json");
  assert.equal(inventory.requiredCases.length, 23);
  assert.equal(new Set(inventory.requiredCases).size, 23);
  assert.equal(inventory.testFiles.length, 4);
  for (const file of [...inventory.testFiles, ...inventory.fixtures]) assert.deepEqual(fileIdentity(resolve(original, file.path)), { sha256: file.sha256, bytes: file.bytes });
  assert.deepEqual(fileIdentity(join(original, inventory.buildScript.path)), { sha256: inventory.buildScript.sha256, bytes: inventory.buildScript.bytes });
  assert.equal(inventory.prerequisites.length, 1);
  assert.equal(inventory.prerequisites[0].executable.path, npm);
  assert.deepEqual(inventory.prerequisites[0].args, ["run", "check:types"]);
  assert(inventory.prerequisites[0].timeoutMs <= receipt.boundsMs.prerequisiteCeiling);
  receipt.additionalRequiredCoverage = inventory.additionalRequiredCoverage;
  sourceBefore = snapshotInputs(original);
  originalBefore = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
  projectBefore = dependencyTree(join(original, "node_modules"));
  npmBefore = dependencyTree(npmRoot);
  assert.deepEqual(sourceBefore, inventory.sourceSnapshot);
  assert.equal(snapshotInputs(join(original, "node_modules"), { exclude: [] }).sha256, inventory.dependencySnapshot.manifestSha256);
  for (const [name, snapshot] of [["source", sourceBefore], ["original", originalBefore], ["project-dependencies", projectBefore], ["npm-runtime", npmBefore]]) save(`${name}-before.json`, snapshot);
  mkdirSync(join(output, "source"));
  for (const file of sourceBefore.files) {
    const destination = join(output, "source", file.path);
    mkdirSync(dirname(destination), { recursive: true });
    copyFileSync(join(original, file.path), destination);
  }
  for (const [path, name] of [[workloadPath, "maintained-workloads.json"], [inventoryPath, "required-tests.json"]]) copyFileSync(path, join(output, name));
  const result = await run("portgate", node, [join(root, "finer/tools/portgate.mjs"), "record", "--arm", "preserved-release-original-remark-breaks", "--ports", "remark-breakslil", "--compiler", compiler, "--codec", codec, "--manifest", workloadPath, "--out", arm, "--build-timeout", "180", "--test-timeout", "90", "--codec-timeout", "60"], root, 430_000);
  assert(completed(result) && [0, 1].includes(result.status), "portgate did not complete; partial evidence retained");
  const manifest = json(join(arm, "manifest.json")), built = json(join(arm, "remark-breakslil.json"));
  assert.deepEqual(manifest.ports, ["remark-breakslil"]);
  assert.deepEqual(manifest.timeoutsMs, { build: 180_000, test: 90_000, codec: 60_000 });
  assert.equal(manifest.compiler.sha256, receipt.parent.compiler.sha256);
  assert.equal(manifest.codec.sha256, receipt.parent.codec.sha256);
  assert.equal(manifest.workloadManifest.sha256, fileIdentity(workloadPath).sha256);
  assert.equal(built.workspace, workspace);
  assert.deepEqual(built.source.snapshot.files, sourceBefore.files);
  workspaceBefore = snapshotInputs(workspace);
  assert.deepEqual(workspaceBefore.files, sourceBefore.files);
  save("workspace-before.json", workspaceBefore);
  receipt.build = built.build;
  receipt.artifacts = built.artifacts;
  receipt.tests = built.tests;
  assert.deepEqual(validateBuild({ exitCode: built.build.status, invocations: built.build.invocations, compilerSha256: manifest.compiler.sha256, contractSha256: built.contractSha256, verifyOutputs: true }), []);
  assert.equal(built.build.trust, "ok");
  assert.equal(built.build.invocations.length, 2);
  for (const [config, file] of [["lilscript.toml", "remark-breaks.raw.js"], ["lilscript.closed.toml", "remark-breaks.closed.js"]]) {
    const invocation = built.build.invocations.find(row => row.output.path === join(workspace, "dist", file));
    assert(invocation, `missing source-built output: ${file}`);
    assert.deepEqual(invocation.args, [join(workspace, "src/entry.lil"), "--target", "js-module", "--config", join(workspace, config), "-o", join(workspace, "dist", file)]);
    assert.equal(invocation.output.previous, null);
    assert.deepEqual(invocation.inputs, sourceBefore.files);
    assert.equal(invocation.configuration.sha256, fileIdentity(join(original, config)).sha256);
  }
  const expectedArtifacts = ["cjs", "closed.js", "esm.js", "raw.js", "umd.js"].map(suffix => `dist/remark-breaks.${suffix}`).sort();
  assert.deepEqual(built.artifacts.map(row => row.path).sort(), expectedArtifacts);
  for (const artifact of built.artifacts) assert.deepEqual(fileIdentity(join(workspace, artifact.path)), { sha256: artifact.sha256, bytes: artifact.raw });
  assert.deepEqual(fileIdentity(join(workspace, "dist/remark-breaks.d.ts")), fileIdentity(join(original, "types/remark-breaks.d.ts")));
  const raw = readFileSync(join(workspace, "dist/remark-breaks.raw.js"), "utf8");
  assert.equal(readFileSync(join(workspace, "dist/remark-breaks.esm.js"), "utf8"), `/*! @itslil/remark-breaks 4.0.1 | LilScript reimplementation of remark-breaks | MIT */\n${raw.trimEnd()}\n`);
  assert.equal(built.build.supervision.timeoutMs, receipt.boundsMs.build);
  assert.equal(built.measurement.supervision.timeoutMs, receipt.boundsMs.codec);
  const report = json(built.tests.report);
  assert.equal(report.supervision?.timeoutMs, receipt.boundsMs.nodeSuite);
  assert.equal(report.suiteExecuted, true);
  assert.deepEqual(report.prerequisites.map(row => row.declaration), inventory.prerequisites);
  for (const row of report.prerequisites) {
    assert(row.attempted && row.status === 0 && !row.signal && !row.error);
    assert.equal(row.supervision.timeoutMs, Math.min(receipt.boundsMs.prerequisiteCeiling, row.declaration.timeoutMs));
    for (const stage of ["before", "after", "final"]) assert(row[stage].every(input => input.matches));
  }
  assert.deepEqual(report.evidence.requiredCases, inventory.requiredCases);
  assert.equal(report.evidence.cases.length, 23);
  assert.deepEqual(validateTests(report.evidence), []);
  assert.deepEqual(report.errors, []);
  const outstanding = inventory.additionalRequiredCoverage.map(coverage => `additional required coverage remains unverified: ${typeof coverage === "string" ? coverage : JSON.stringify(coverage)}`);
  assert.deepEqual(built.tests.errors, outstanding);
  assert.equal(result.status, outstanding.length ? 1 : 0);
  receipt.originalTestBoundaryPassed = true;
  save("receipt.json", receipt);
  const pack = await run("original-check-pack", npm, ["run", "check:pack"], workspace, 60_000);
  receipt.packageDryRunPassed = completed(pack) && pack.status === 0;
  assert(receipt.packageDryRunPassed, "original package dry-run check did not pass");
  for (const artifact of built.artifacts) assert.deepEqual(fileIdentity(join(workspace, artifact.path)), { sha256: artifact.sha256, bytes: artifact.raw });
  assert.deepEqual(fileIdentity(join(workspace, "dist/remark-breaks.d.ts")), fileIdentity(join(original, "types/remark-breaks.d.ts")));
  const paths = built.artifacts.map(row => join(workspace, row.path));
  const measured = await run("canonical-codec", codec, ["--json", ...paths], workspace, 60_000);
  assert(completed(measured) && measured.status === 0, "canonical codec replay did not pass");
  const sizes = validateMeasurements(JSON.parse(measured.stdout), paths);
  for (const artifact of built.artifacts) for (const key of ["raw", "gzip9", "brotli11"]) assert.equal(sizes.get(join(workspace, artifact.path))[key], artifact[key]);
  receipt.passed = true;
  receipt.outcome = "Original source-built npm-test boundary and separate original package dry-run passed; additional delivery and browser/site requirements remain unverified.";
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const after = pins();
    save("inputs-after.json", after);
    receipt.inputsStable = initialPins?.sha256 === after.sha256;
    assert.equal(receipt.inputsStable, true, "evidence inputs changed");
    for (const [name, before, after] of [
      ["source", sourceBefore, sourceBefore && snapshotInputs(original)],
      ["original", originalBefore, originalBefore && snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] })],
      ["project-dependencies", projectBefore, projectBefore && dependencyTree(join(original, "node_modules"))],
      ["npm-runtime", npmBefore, npmBefore && dependencyTree(npmRoot)],
      ["workspace", workspaceBefore, workspaceBefore && snapshotInputs(workspace)],
    ]) if (before) { save(`${name}-after.json`, after); assert.deepEqual(after, before, `${name} changed`); }
    if (existsSync(join(arm, "lilscript"))) assert.deepEqual(fileIdentity(join(arm, "lilscript")), fileIdentity(compiler));
    if (existsSync(join(arm, "lilscript-codec"))) assert.deepEqual(fileIdentity(join(arm, "lilscript-codec")), fileIdentity(codec));
  } catch (error) {
    receipt.passed = receipt.originalTestBoundaryPassed = receipt.packageDryRunPassed = false;
    receipt.validationFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  try {
    if (existsSync(arm)) {
      for (const name of ["manifest.json", "remark-breakslil.json", "logs", "wrappers", "invocations", "tests"]) if (existsSync(join(arm, name))) cpSync(join(arm, name), join(output, name), { recursive: true });
      if (existsSync(join(workspace, "dist"))) cpSync(join(workspace, "dist"), join(output, "artifacts"), { recursive: true });
    }
  } catch (error) {
    receipt.passed = receipt.originalTestBoundaryPassed = receipt.packageDryRunPassed = false;
    receipt.archiveFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.elapsedMs = elapsed();
  receipt.loadAverageEnd = loadavg();
  receipt.completed = new Date().toISOString();
  try { receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files; }
  catch (error) {
    receipt.passed = receipt.originalTestBoundaryPassed = receipt.packageDryRunPassed = false;
    receipt.outputManifestFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  save("receipt.json", receipt);
  console.log(JSON.stringify({ output, passed: receipt.passed, originalTestBoundaryPassed: receipt.originalTestBoundaryPassed, packageDryRunPassed: receipt.packageDryRunPassed, qualification: receipt.qualification, inputsStable: receipt.inputsStable, failure: receipt.failure?.message, validationFailure: receipt.validationFailure?.message }));
}
