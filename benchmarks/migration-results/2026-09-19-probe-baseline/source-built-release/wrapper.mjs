import assert from "node:assert/strict";
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { loadavg, tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { digest, fileIdentity, fingerprint, snapshotInputs, validateBuild, validateInvocation, validateMeasurements, validateTests } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");
assert.equal(process.argv.length, 3, "usage: qualify.mjs new-output-directory");
const output = resolve(process.argv[2]);
mkdirSync(output, { recursive: false });
const original = "/home/azureuser/probelil";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const preserved = "/tmp/lilscript-public-integration-release-baseline-20260919";
const compiler = join(preserved, "lilscript");
const codec = join(preserved, "lilscript-codec");
const parentPath = join(root, "benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-19T16-36-23.430Z/receipt.json");
const workloadPath = join(root, "benchmarks/libraries/maintained-workloads.json");
const inventoryPath = join(root, "benchmarks/libraries/probelil.required-tests.json");
const fixturePath = join(root, "finer/tools/port-adapters/probe.test.mjs");
const control = join(root, "tests/config/no-optimization.toml");
const toolNames = ["portgate", "artifact-evidence", "compiler-receipt", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts", "vitest-test-evidence"];
const toolPaths = toolNames.map(name => join(root, `finer/tools/${name}.mjs`));
const profiles = ["shipped", "level0", "level5", "level9", "level15", "searchOff", "searchAlways", "gzip", "raw", "perfFirst", "balanced", "noPeephole", "arrows", "functions", "looseNames", "beam1", "beam32", "freqNames", "freqNamesSearchOff", "idiomNames", "idiomNamesSearchOff", "phiRegions"];
const knownRejectedConfigurationProfiles = ["freqNames", "freqNamesSearchOff", "idiomNames", "idiomNamesSearchOff"];
const json = path => JSON.parse(readFileSync(path, "utf8"));
const identify = path => ({ path, ...fileIdentity(path) });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const inputs = [fileURLToPath(import.meta.url), node, compiler, codec, parentPath, ...["inputs-before.json", "inputs-after.json"].map(name => join(dirname(parentPath), name)), workloadPath, inventoryPath, fixturePath, control, join(root, ".nvmrc"), ...toolPaths];
const pins = () => {
  const files = inputs.map(identify);
  return { files, sha256: fingerprint(files) };
};
const started = process.hrtime.bigint();
const elapsed = () => Number(process.hrtime.bigint() - started) / 1e6;
const timeout = ceiling => {
  const remaining = Math.floor(370_000 - elapsed());
  assert(remaining > 0, "shared 370-second command budget exhausted");
  return Math.min(ceiling, remaining);
};
const removed = Object.keys(process.env).filter(key => key.startsWith("LILSCRIPT_"));
const overrides = { PATH: "/home/azureuser/.nvm/versions/node/v24.11.1/bin:/usr/local/bin:/usr/bin:/bin", NODE_OPTIONS: "", NODE_PATH: "", RAYON_NUM_THREADS: "2", npm_config_offline: "true", npm_config_ignore_scripts: "true" };
const env = { ...process.env, ...overrides };
for (const key of [...removed, "NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"]) delete env[key];
const receipt = {
  schema: 1, kind: "source-pinned-original-probe-baseline", started: new Date().toISOString(),
  passed: false, evidenceComplete: false, qualification: "unverified", commands: [],
  profiles, knownRejectedConfigurationProfiles, environment: { overrides, removed: [...removed, "NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"], otherwise: "inherited" },
  boundsMs: { sharedCommands: 370_000, externalSupervisor: 390_000, baseSupervisor: 120_000, baseEachPhase: 90_000, matrix: 240_000, measurement: 60_000 },
  scope: "Original two golden lanes and all 22 original matrix profiles using a preserved accepted release compiler; no changed configuration or expected output",
  limitations: [
    "The preserved legacy compiler is tied to its accepted source manifest, not current changing compiler sources or semantic-backend support.",
    "Four original name_ordering profiles are expected to reject configuration; no boolean replacement or historical altered-script pass credit is used.",
    "Matrix rows are original script dispositions plus compiler/load/codec receipts, not newly claimed frozen Node test identities. The base adapter retains its original two identities.",
    "A failed profile remains required. Complete evidence of a failed attempt is not a passing canary gate or a language-wide/library-size claim.",
    "Original matrix execution captures stdout internally and compares it with expected.out. Matching pass lines plus exact module-load evidence bind that original assertion; raw successful child stdout is not separately retained.",
    "The original matrix summary subtracts compilation failures from its run count; per-profile statuses, not its aggregate prose, determine counts here.",
    "Canonical file scores include the required host runner separately; they are not competitive wins or a deployment optimum.",
    "Concurrent host load is not isolated. No speed claim; command supervision is not a hard OS resource bound and excludes synchronous hashing/copying, escaped sessions and supervisor SIGKILL cleanup.",
  ],
  loadAverageStart: loadavg(),
};
let initialPins, sourceBefore, originalBefore, workspaceBefore, arm, workspace, matrixDirectory, matrixLoaded;
let matrixResult, matrixContract, inventory;
save("receipt.json", receipt);

async function run(label, command, args, cwd, commandEnv, ceiling) {
  const begin = process.hrtime.bigint();
  const result = await runBoundedCommand(command, args, { cwd, env: commandEnv, timeoutMs: timeout(ceiling), maxBuffer: 32 * 1024 * 1024 });
  writeFileSync(join(output, `${label}.stdout`), result.stdout);
  writeFileSync(join(output, `${label}.stderr`), result.stderr);
  receipt.commands.push({ label, command, args, cwd, status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision, wall_ns: Number(process.hrtime.bigint() - begin) });
  save("receipt.json", receipt);
  return result;
}

function completed(result) {
  return result && !result.error && !result.signal && !result.supervision.timedOut;
}

function collectMatrix() {
  if (!matrixDirectory || !existsSync(matrixDirectory)) return;
  const invocations = readdirSync(matrixDirectory).filter(name => name.endsWith(".json")).sort().map(name => json(join(matrixDirectory, name)));
  const loads = matrixLoaded && existsSync(matrixLoaded) ? readdirSync(matrixLoaded).filter(name => name.endsWith(".json")).sort().map(name => json(join(matrixLoaded, name))) : [];
  const runnerLoads = loads.filter(row => row.path === join(workspace, "dist/run.cjs"));
  const runner = fileIdentity(join(workspace, "dist/run.cjs"));
  const lines = `${matrixResult?.stdout ?? ""}\n${matrixResult?.stderr ?? ""}`.split("\n");
  const rows = profiles.map(profile => {
    const config = join(workspace, `dist/configs/${profile}.toml`);
    const artifact = join(workspace, `dist/configs/${profile}.js`);
    const matches = invocations.filter(row => row.output?.path === artifact);
    const invocation = matches.length === 1 ? matches[0] : null;
    const prefix = `  ${profile.padEnd(14)} `;
    const reported = lines.filter(line => line.startsWith(prefix));
    const statement = reported.length === 1 ? reported[0].slice(prefix.length) : null;
    const disposition = statement?.startsWith("DID NOT COMPILE:") ? "compile-failed" : statement?.startsWith("THREW:") ? "runtime-failed" : statement?.startsWith("WRONG OUTPUT (") ? "golden-mismatch" : /^ok  \d+ B$/.test(statement ?? "") ? "golden-match" : "unreported";
    const artifactIdentity = existsSync(artifact) ? identify(artifact) : null;
    const observed = loads.filter(row => row.path === artifact);
    const errors = [];
    if (matches.length !== 1) errors.push(`expected one invocation, got ${matches.length}`);
    if (reported.length !== 1 || disposition === "unreported") errors.push(`expected one original disposition, got ${reported.length}`);
    if (invocation) {
      assert.equal(invocation.compiler.sha256, receipt.parent.compiler.sha256);
      assert.equal(invocation.contractSha256, matrixContract);
      assert.equal(invocation.inputSha256, invocation.inputAfterSha256);
      assert.equal(invocation.compilerAfterSha256, receipt.parent.compiler.sha256);
      assert.deepEqual(invocation.inputs, workspaceBefore.files);
      assert.deepEqual(invocation.args, [join(workspace, "src/probe.lil"), "--target", "js", "--config", config, "-o", artifact]);
      assert.equal(invocation.output.previous, null);
      assert.deepEqual({ sha256: invocation.configuration.sha256, bytes: invocation.configuration.bytes }, fileIdentity(config));
      const configuration = { path: invocation.configuration.path, sha256: invocation.configuration.sha256, bytes: invocation.configuration.bytes };
      assert.equal(fingerprint({ files: invocation.inputs, configuration }), invocation.inputSha256);
    }
    const compilePassed = invocation?.exitCode === 0 && !invocation.signal && !invocation.error;
    const compileErrors = invocation ? validateInvocation(invocation, { compilerSha256: receipt.parent.compiler.sha256, contractSha256: matrixContract, verifyOutput: true }) : [];
    if (compilePassed && compileErrors.length) errors.push(...compileErrors);
    if (compilePassed && !artifactIdentity) errors.push("completed compiler output missing");
    if (!compilePassed && (artifactIdentity || observed.length || disposition !== "compile-failed")) errors.push("failed compile has inconsistent artifact/runtime evidence");
    if (compilePassed && disposition === "compile-failed") errors.push("original script and compiler exit disagree");
    if (compilePassed && !observed.length) errors.push("completed output was not observed executing");
    if (artifactIdentity && observed.some(row => row.sha256 !== artifactIdentity.sha256 || row.bytes !== artifactIdentity.bytes)) errors.push("executed bytes differ from retained output");
    if (observed.some(row => !runnerLoads.some(load => load.pid === row.pid && load.sha256 === runner.sha256 && load.bytes === runner.bytes))) errors.push("profile execution has no matching exact host-runner load in the same process");
    if (disposition === "golden-match" && artifactIdentity && Number(statement.match(/^ok  (\d+) B$/)[1]) !== artifactIdentity.bytes) errors.push("original script byte count differs");
    return { profile, configuration: existsSync(config) ? identify(config) : null, artifact: artifactIdentity, invocation, originalDisposition: disposition, originalLines: reported, compilePassed, compileErrors, observedLoads: observed, goldenMatched: compilePassed && disposition === "golden-match" && observed.length > 0 && errors.length === 0, evidenceErrors: errors };
  });
  const unexpectedInvocations = invocations.filter(row => !rows.some(profile => profile.invocation?.id === row.id));
  const unexpectedLoads = loads.filter(row => row.path !== join(workspace, "dist/run.cjs") && !rows.some(profile => profile.artifact?.path === row.path));
  assert(runnerLoads.every(row => row.sha256 === runner.sha256 && row.bytes === runner.bytes));
  receipt.matrix = { contractSha256: matrixContract, rows, unexpectedInvocations, unexpectedLoads, runnerLoads, commandCompleted: completed(matrixResult), exitCode: matrixResult?.status ?? null,
    counts: { requested: profiles.length, invocationReceipts: invocations.length, compiled: rows.filter(row => row.compilePassed).length, observedProfiles: rows.filter(row => row.observedLoads.length).length, goldenMatches: rows.filter(row => row.goldenMatched).length, compilationFailures: rows.filter(row => row.originalDisposition === "compile-failed").length, runtimeFailures: rows.filter(row => ["runtime-failed", "golden-mismatch"].includes(row.originalDisposition)).length, evidenceErrors: rows.reduce((count, row) => count + row.evidenceErrors.length, 0) },
  };
  save("matrix.json", receipt.matrix);
}

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
  assert(parent.passed && parent.inputsStable && parent.profile === "release");
  copyFileSync(parentPath, join(output, "parent-receipt.json"));
  for (const name of ["inputs-before.json", "inputs-after.json"]) {
    const path = join(dirname(parentPath), name), input = json(path);
    assert.equal(digest(JSON.stringify(input.files)), parent.inputSha256);
    assert.equal(input.sha256, parent.inputSha256);
    const pinned = input.files.find(row => row.path === "tests/config/no-optimization.toml");
    assert.deepEqual(fileIdentity(control), { sha256: pinned.sha256, bytes: pinned.bytes });
    copyFileSync(path, join(output, `parent-${name}`));
  }
  for (const path of [compiler, codec]) {
    const row = parent.binaries.find(row => basename(row.path) === basename(path));
    assert(row, `parent binary missing: ${path}`);
    assert.deepEqual(fileIdentity(path), { sha256: row.sha256, bytes: row.bytes });
  }
  receipt.parent = { ...identify(parentPath), inputSha256: parent.inputSha256, compiler: identify(compiler), codec: identify(codec), externalControl: identify(control) };
  inventory = json(inventoryPath);
  assert.equal(inventory.workload, "probelil");
  assert.equal(inventory.requiredCases.length, 2);
  assert.equal(new Set(inventory.requiredCases).size, 2);
  assert.deepEqual(fileIdentity(fixturePath), { sha256: inventory.testFiles[0].sha256, bytes: inventory.testFiles[0].bytes });
  assert.deepEqual(fileIdentity(join(original, "scripts/build.mjs")), { sha256: inventory.buildScript.sha256, bytes: inventory.buildScript.bytes });
  assert.deepEqual(fileIdentity(join(original, "scripts/configs.mjs")), inventory.additionalRequiredCoverage[0].identity);
  assert.equal(inventory.additionalRequiredCoverage[0].profiles, profiles.length);
  const golden = inventory.fixtures.find(row => row.path === "expected.out");
  assert.deepEqual(fileIdentity(join(original, "expected.out")), { sha256: golden.sha256, bytes: golden.bytes });
  assert.equal(existsSync(join(original, "node_modules")), false);
  sourceBefore = snapshotInputs(original);
  originalBefore = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
  save("source-before.json", sourceBefore);
  save("original-before.json", originalBefore);
  mkdirSync(join(output, "source"));
  for (const file of sourceBefore.files) {
    const dest = join(output, "source", file.path);
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(join(original, file.path), dest);
  }
  copyFileSync(inventoryPath, join(output, "required-base-tests.json"));
  copyFileSync(workloadPath, join(output, "maintained-workloads.json"));
  copyFileSync(fixturePath, join(output, "probe.test.mjs"));
  copyFileSync(control, join(output, "no-optimization.toml"));
  arm = join(mkdtempSync(join(tmpdir(), "lilscript-probe-baseline-20260919-")), "portgate");
  workspace = join(arm, "workspaces", "probelil");
  receipt.arm = arm;
  const baseResult = await run("base-portgate", node, [join(root, "finer/tools/portgate.mjs"), "record", "--arm", "preserved-release-original-probe", "--ports", "probelil", "--compiler", compiler, "--codec", codec, "--manifest", workloadPath, "--out", arm, "--timeout", "90"], root, env, 120_000);
  const base = json(join(arm, "probelil.json")), manifest = json(join(arm, "manifest.json"));
  assert.deepEqual(manifest.ports, ["probelil"]);
  assert.equal(manifest.compiler.sha256, receipt.parent.compiler.sha256);
  assert.equal(manifest.codec.sha256, receipt.parent.codec.sha256);
  assert.equal(manifest.workloadManifest.sha256, fileIdentity(workloadPath).sha256);
  assert.deepEqual(base.source.snapshot.files, sourceBefore.files);
  assert.equal(base.workspace, workspace);
  workspaceBefore = snapshotInputs(workspace);
  save("workspace-before.json", workspaceBefore);
  const expectedInputs = [...sourceBefore.files, { path: ".lilscript-test-adapter/suite.test.mjs", kind: "file", executable: false, ...fileIdentity(fixturePath) }].sort((a, b) => a.path.localeCompare(b.path, "en"));
  assert.deepEqual(new Map(workspaceBefore.files.map(row => [row.path, row])), new Map(expectedInputs.map(row => [row.path, row])));
  const baseErrors = validateBuild({ exitCode: base.build.status, invocations: base.build.invocations, compilerSha256: manifest.compiler.sha256, contractSha256: base.contractSha256, verifyOutputs: true });
  if (!completed(baseResult) || ![0, 1].includes(baseResult.status)) baseErrors.push("base supervisor did not complete");
  if (base.build.invocations.length !== 2) baseErrors.push("expected two original base compiler invocations");
  for (const invocation of base.build.invocations) {
    const optimized = invocation.output.path === join(workspace, "dist/probe.js");
    const expectedConfig = optimized ? join(workspace, "lilscript.toml") : control;
    assert.deepEqual(invocation.args, [join(workspace, "src/probe.lil"), "--target", "js", "--config", expectedConfig, "-o", join(workspace, optimized ? "dist/probe.js" : "dist/probe.none.js")]);
    assert.deepEqual(invocation.inputs, workspaceBefore.files);
  }
  let baseReport = null;
  if (base.tests?.report && existsSync(base.tests.report)) {
    baseReport = json(base.tests.report);
    assert.deepEqual(baseReport.evidence.requiredCases, inventory.requiredCases);
    assert.deepEqual(baseReport.evidence.cases.map(row => row.id).sort(), [...inventory.requiredCases].sort());
    baseErrors.push(...validateTests(baseReport.evidence), ...baseReport.errors);
    const outstanding = inventory.additionalRequiredCoverage.map(row => `additional required coverage remains unverified: ${JSON.stringify(row)}`);
    assert.deepEqual(base.tests.errors, [...baseReport.errors, ...outstanding]);
  } else baseErrors.push("original two-case evidence was not reached");
  receipt.base = { commandCompleted: completed(baseResult), exitCode: baseResult.status, errors: baseErrors, qualified: baseErrors.length === 0, originalRecordCertification: base.tests?.certification ?? null, requiredCases: inventory.requiredCases, artifacts: base.artifacts, testReport: baseReport ? identify(base.tests.report) : null };
  save("receipt.json", receipt);
  assert.equal(existsSync(join(workspace, "dist/configs")), false, "matrix must start without stale outputs");
  matrixDirectory = join(arm, "matrix-invocations");
  matrixLoaded = join(arm, "matrix-loaded");
  mkdirSync(matrixDirectory);
  matrixContract = fingerprint({ source: sourceBefore.sha256, matrix: identify(join(original, "scripts/configs.mjs")), command: ["scripts/configs.mjs", "--keep"], profiles });
  const settings = join(arm, "matrix-wrapper.json");
  save("required-matrix-profiles.json", { schema: 1, profiles, script: identify(join(original, "scripts/configs.mjs")), golden, knownRejectedConfigurationProfiles, scope: "Original profile identities fixed before execution, not frozen Node test IDs" });
  writeFileSync(settings, JSON.stringify({ compiler: join(arm, "lilscript"), compilerSha256: receipt.parent.compiler.sha256, contractSha256: matrixContract, receiptDirectory: matrixDirectory, outputRoots: ["dist"] }));
  const wrapper = join(arm, "matrix-compiler.mjs");
  writeFileSync(wrapper, `#!${node}\nimport {runCompilerReceipt} from ${JSON.stringify(pathToFileURL(join(root, "finer/tools/compiler-receipt.mjs")).href)};\ntry { process.exitCode=runCompilerReceipt(${JSON.stringify(settings)},process.argv.slice(2)); } catch(error) { console.error(error.message); process.exitCode=1; }\n`, { mode: 0o755 });
  const preload = join(arm, "matrix-observe.mjs");
  const paths = profiles.map(profile => join(workspace, `dist/configs/${profile}.js`));
  writeFileSync(preload, `import {observeNodeArtifacts} from ${JSON.stringify(pathToFileURL(join(root, "finer/tools/observe-node-artifacts.mjs")).href)};\nobserveNodeArtifacts(${JSON.stringify({ paths: [...paths, join(workspace, "dist/run.cjs")], directory: matrixLoaded })});\n`);
  receipt.matrixOwnerInputs = [settings, wrapper, preload].map(identify);
  matrixResult = await run("matrix", node, ["scripts/configs.mjs", "--keep"], workspace, { ...env, LILSCRIPT_ROOT: root, LILSCRIPT_COMPILER: wrapper, NODE_OPTIONS: `--import=${pathToFileURL(preload).href}` }, 240_000);
  collectMatrix();
  const present = receipt.matrix.rows.filter(row => row.compilePassed && row.artifact).map(row => row.artifact.path);
  const measurementPaths = [...new Set([...base.artifacts.map(row => join(workspace, row.path)), ...present, join(workspace, "dist/run.cjs")])].filter(existsSync);
  const measured = await run("canonical-codec", codec, ["--json", ...measurementPaths], workspace, env, 60_000);
  assert(completed(measured) && measured.status === 0, "canonical measurement failed");
  const scores = validateMeasurements(JSON.parse(measured.stdout), measurementPaths);
  receipt.artifacts = [...scores].map(([path, sizes]) => ({ ...identify(path), ...sizes }));
  for (const artifact of receipt.artifacts) {
    const originalScore = base.artifacts.find(row => join(workspace, row.path) === artifact.path);
    if (originalScore) for (const key of ["raw", "gzip9", "brotli11"]) assert.equal(originalScore[key], artifact[key]);
  }
  receipt.evidenceComplete = completed(matrixResult) && [0, 1].includes(matrixResult.status) && receipt.matrix.rows.every(row => row.evidenceErrors.length === 0) && receipt.matrix.unexpectedInvocations.length === 0 && receipt.matrix.unexpectedLoads.length === 0;
  receipt.passed = receipt.base.qualified && receipt.evidenceComplete && matrixResult.status === 0 && receipt.matrix.counts.goldenMatches === profiles.length;
  receipt.qualification = receipt.passed ? "verified-original-probe-canary-baseline" : "unverified";
  if (!receipt.passed) process.exitCode = 1;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    if (matrixResult) collectMatrix();
    const after = pins();
    save("inputs-after.json", after);
    receipt.inputsStable = initialPins?.sha256 === after.sha256;
    assert.equal(receipt.inputsStable, true);
    if (sourceBefore) {
      const after = snapshotInputs(original);
      save("source-after.json", after);
      assert.deepEqual(after, sourceBefore);
      const full = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
      save("original-after.json", full);
      assert.deepEqual(full, originalBefore);
    }
    if (workspaceBefore) {
      const after = snapshotInputs(workspace);
      save("workspace-after.json", after);
      assert.deepEqual(after, workspaceBefore);
    }
    if (receipt.matrixOwnerInputs) for (const row of receipt.matrixOwnerInputs) assert.deepEqual(fileIdentity(row.path), { sha256: row.sha256, bytes: row.bytes });
    if (arm && existsSync(join(arm, "lilscript"))) assert.deepEqual(fileIdentity(join(arm, "lilscript")), fileIdentity(compiler));
    if (arm && existsSync(join(arm, "lilscript-codec"))) assert.deepEqual(fileIdentity(join(arm, "lilscript-codec")), fileIdentity(codec));
  } catch (error) {
    receipt.passed = receipt.evidenceComplete = false;
    receipt.qualification = "unverified";
    receipt.validationFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  try {
    if (arm && existsSync(arm)) {
      for (const name of ["manifest.json", "probelil.json", "logs", "wrappers", "invocations", "tests", "matrix-invocations", "matrix-loaded", "matrix-wrapper.json", "matrix-compiler.mjs", "matrix-observe.mjs"]) if (existsSync(join(arm, name))) cpSync(join(arm, name), join(output, name), { recursive: true });
      if (workspace && existsSync(join(workspace, "dist"))) cpSync(join(workspace, "dist"), join(output, "artifacts"), { recursive: true });
    }
  } catch (error) {
    receipt.passed = receipt.evidenceComplete = false;
    receipt.qualification = "unverified";
    receipt.archiveFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.elapsedMs = elapsed();
  receipt.loadAverageEnd = loadavg();
  receipt.completed = new Date().toISOString();
  try {
    receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files;
  } catch (error) {
    receipt.passed = receipt.evidenceComplete = false;
    receipt.qualification = "unverified";
    receipt.outputManifestFailure = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  save("receipt.json", receipt);
  console.log(JSON.stringify({ output, passed: receipt.passed, evidenceComplete: receipt.evidenceComplete, baseQualified: receipt.base?.qualified, matrix: receipt.matrix?.counts, failure: receipt.failure?.message, validationFailure: receipt.validationFailure?.message }));
}
