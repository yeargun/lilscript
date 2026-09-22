import assert from "node:assert/strict";
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { loadavg, tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { digest, fileIdentity, fingerprint, snapshotInputs, validateBuild, validateMeasurements, validateTests } from "./artifact-evidence.mjs";
import { runBoundedCommand } from "./bounded-command.mjs";

const ownerPath = fileURLToPath(import.meta.url);
const root = resolve(dirname(ownerPath), "../..");
const textValue = value => typeof value === "string" && value.trim().length > 0 && !value.includes("\0");

function record(value, keys, label) {
  assert(value && typeof value === "object" && !Array.isArray(value) &&
    [Object.prototype, null].includes(Object.getPrototypeOf(value)), `${label} must be a plain data record`);
  const descriptors = Object.getOwnPropertyDescriptors(value);
  assert.deepEqual(Reflect.ownKeys(value).sort(), [...keys].sort(), `${label} has missing or unknown keys`);
  for (const key of keys) assert("value" in descriptors[key], `${label} must not contain accessors`);
}

function validateRecipe(recipe) {
  record(recipe, ["workload", "workspace", "inventoryPath", "discovery", "caseCount", "testFileCount",
    "artifactStem", "esmBanner", "kind", "armLabel", "scope", "limitations", "defaultParent"], "recipe");
  record(recipe.discovery, ["directory", "sha256"], "recipe.discovery");
  record(recipe.defaultParent, ["receipt", "sha256", "compiler", "codec"], "recipe.defaultParent");
  for (const key of ["workload", "workspace", "inventoryPath", "artifactStem", "esmBanner", "kind", "armLabel", "scope"])
    assert(textValue(recipe[key]), `recipe.${key} must be nonempty text`);
  for (const key of ["directory", "sha256"]) assert(textValue(recipe.discovery[key]), `recipe.discovery.${key} must be nonempty text`);
  for (const key of ["receipt", "sha256", "compiler", "codec"])
    assert(textValue(recipe.defaultParent[key]), `recipe.defaultParent.${key} must be nonempty text`);
  for (const key of ["workload", "artifactStem"])
    assert(/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(recipe[key]), `recipe.${key} must be a single name`);
  assert(isAbsolute(recipe.workspace), "recipe.workspace must be absolute");
  for (const hash of [recipe.discovery.sha256, recipe.defaultParent.sha256])
    assert(/^[a-f0-9]{64}$/.test(hash), "recipe SHA-256 must be 64 lowercase hexadecimal characters");
  for (const key of ["caseCount", "testFileCount"])
    assert(Number.isSafeInteger(recipe[key]) && recipe[key] > 0, `recipe.${key} must be a positive integer`);
  assert(recipe.esmBanner.endsWith("\n"), "recipe.esmBanner must include its trailing newline");
  assert(Array.isArray(recipe.limitations) && recipe.limitations.length > 0 && recipe.limitations.every(textValue),
    "recipe.limitations must be nonempty text entries");
}

// The recipe only names pinned inputs and expected outputs; all admission gates
// and resource bounds remain mandatory in this owner.
export async function qualifyNodeLibrary(recipe, argv, entryPath) {
  validateRecipe(recipe);
  assert(Array.isArray(argv) && argv.every(value => typeof value === "string" && !value.includes("\0")),
    "argv must contain strings");
  assert(textValue(entryPath), "entryPath must be nonempty text");
  recipe = JSON.parse(JSON.stringify(recipe));
  entryPath = resolve(entryPath);
  const [outputArgument, ...arguments_] = argv;
  assert(typeof outputArgument === "string" && outputArgument.trim() && !outputArgument.startsWith("--"),
    "usage: qualify.mjs new-output-directory [--parent-receipt path --parent-sha256 hash] [--compiler path] [--codec path]");
  const keys = new Set(["--parent-receipt", "--parent-sha256", "--compiler", "--codec"]), options = new Map();
  for (let index = 0; index < arguments_.length; index += 2) {
    const key = arguments_[index], value = arguments_[index + 1];
    assert(keys.has(key), `unknown option: ${key}`);
    assert(!options.has(key), `duplicate option: ${key}`);
    assert(typeof value === "string" && value.trim() && !value.startsWith("--"), `nonempty value required: ${key}`);
    options.set(key, value);
  }
  assert.equal(options.has("--parent-receipt"), options.has("--parent-sha256"), "parent receipt and expected SHA-256 must be provided together");
  const parentSha256 = options.get("--parent-sha256") ?? recipe.defaultParent.sha256;
  assert(/^[a-f0-9]{64}$/.test(parentSha256), "parent SHA-256 must be 64 lowercase hexadecimal characters");
  const output = resolve(outputArgument);
  mkdirSync(output, { recursive: false });
  const original = recipe.workspace;
  const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
  const npm = join(dirname(node), "npm"), npmRoot = dirname(dirname(realpathSync(npm)));
  const compiler = resolve(options.get("--compiler") ?? resolve(root, recipe.defaultParent.compiler));
  const codec = resolve(options.get("--codec") ?? resolve(root, recipe.defaultParent.codec));
  const parentPath = resolve(options.get("--parent-receipt") ?? resolve(root, recipe.defaultParent.receipt));
  const workloadPath = join(root, "benchmarks/libraries/maintained-workloads.json");
  const inventoryPath = resolve(root, recipe.inventoryPath);
  const discoveryDirectory = resolve(root, recipe.discovery.directory);
  const discoveryPaths = ["receipt.json", "before.json", "node/report.json"].map(name => join(discoveryDirectory, name));
  const toolPaths = ["portgate", "artifact-evidence", "compiler-receipt", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts", "vitest-test-evidence"].map(name => join(root, `finer/tools/${name}.mjs`));
  const scratch = mkdtempSync(join(tmpdir(), `lilscript-${recipe.workload}-baseline-`));
  const arm = join(scratch, "portgate"), workspace = join(arm, "workspaces", recipe.workload);
  const userConfig = join(output, "npm-user.npmrc"), globalConfig = join(output, "npm-global.npmrc");
  writeFileSync(userConfig, "");
  writeFileSync(globalConfig, "");
  const identify = path => ({ path, ...fileIdentity(path) });
  const json = path => JSON.parse(readFileSync(path, "utf8"));
  const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
  const evidencePaths = [entryPath, ownerPath, node, npm, compiler, codec, parentPath, ...["inputs-before.json", "inputs-after.json"].map(name => join(dirname(parentPath), name)), workloadPath, inventoryPath, ...discoveryPaths, join(root, ".nvmrc"), userConfig, globalConfig, ...toolPaths];
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
    const milliseconds = Math.floor(600_000 - elapsed());
    assert(milliseconds > 0, "shared 600-second command budget exhausted");
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
    schema: 1, kind: recipe.kind, recipe, started: new Date().toISOString(), arm, workspace,
    requestedParent: { path: parentPath, sha256: parentSha256, compiler, codec, overrides: Object.fromEntries(options) },
    passed: false, originalTestBoundaryPassed: false, packageDryRunPassed: false, qualification: "unverified", commands: [],
    boundsMs: { build: 300_000, nodeSuite: 90_000, prerequisiteCeiling: 90_000, codec: 60_000, pack: 60_000, sharedCommands: 600_000, externalSupervisor: 630_000 },
    environment: { overrides, removed, otherwise: "inherited; original workspace .npmrc preserved if present" },
    scope: recipe.scope,
    limitations: recipe.limitations, loadAverageStart: loadavg(),
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
    copyFileSync(entryPath, join(output, "wrapper.mjs"));
    copyFileSync(ownerPath, join(output, "qualification-owner.mjs"));
    mkdirSync(join(output, "runner-sources"));
    for (const path of toolPaths) copyFileSync(path, join(output, "runner-sources", basename(path)));
    assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
    assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
    assert.equal(fileIdentity(parentPath).sha256, parentSha256);
    const parent = json(parentPath);
    assert(parent.passed === true && parent.inputsStable === true && parent.profile === "release" && parent.publicIntegration === true);
    assert(typeof parent.inputSha256 === "string" && /^[a-f0-9]{64}$/.test(parent.inputSha256));
    copyFileSync(parentPath, join(output, "parent-receipt.json"));
    for (const name of ["inputs-before.json", "inputs-after.json"]) {
      const path = join(dirname(parentPath), name), input = json(path);
      assert.equal(digest(JSON.stringify(input.files)), parent.inputSha256);
      assert.equal(input.sha256, parent.inputSha256);
      copyFileSync(path, join(output, `parent-${name}`));
    }
    for (const [path, name] of [[compiler, "lilscript"], [codec, "lilscript-codec"]]) {
      const rows = parent.binaries.filter(row => basename(row.path) === name);
      assert.equal(rows.length, 1, `missing or ambiguous accepted binary: ${name}`);
      const row = rows[0];
      assert.deepEqual(fileIdentity(path), { sha256: row.sha256, bytes: row.bytes });
    }
    receipt.parent = { ...identify(parentPath), inputSha256: parent.inputSha256, compiler: identify(compiler), codec: identify(codec) };
    const inventory = json(inventoryPath), workload = json(workloadPath).libraries.find(row => row.id === recipe.workload);
    assert(workload, `missing maintained workload: ${recipe.workload}`);
    assert.equal(workload.workspace, original);
    assert.equal(workload.tests.nodeAdapter.caseInventory, basename(inventoryPath));
    assert.equal(fileIdentity(discoveryPaths[0]).sha256, recipe.discovery.sha256);
    const discovery = json(discoveryPaths[0]);
    assert(discovery.passed && discovery.inputsStable && discovery.sourceBuilt === false);
    assert.deepEqual(fileIdentity(resolve(root, inventory.capturedFrom.path)), { sha256: inventory.capturedFrom.sha256, bytes: inventory.capturedFrom.bytes });
    assert.deepEqual(fileIdentity(resolve(root, inventory.dependencySnapshot.path)), { sha256: inventory.dependencySnapshot.sha256, bytes: inventory.dependencySnapshot.bytes });
    assert.deepEqual(json(discoveryPaths[2]).evidence.cases.map(row => row.id).sort(), inventory.requiredCases);
    for (const [path, name] of [[discoveryPaths[0], "discovery-receipt.json"], [discoveryPaths[1], "discovery-before.json"], [discoveryPaths[2], "discovery-report.json"]]) copyFileSync(path, join(output, name));
    assert.equal(inventory.requiredCases.length, recipe.caseCount);
    assert.equal(new Set(inventory.requiredCases).size, recipe.caseCount);
    assert.equal(inventory.testFiles.length, recipe.testFileCount);
    for (const file of [...inventory.testFiles, ...inventory.fixtures]) assert.deepEqual(fileIdentity(resolve(original, file.path)), { sha256: file.sha256, bytes: file.bytes });
    assert.deepEqual(fileIdentity(join(original, inventory.buildScript.path)), { sha256: inventory.buildScript.sha256, bytes: inventory.buildScript.bytes });
    assert.equal(inventory.prerequisites.length, 1);
    assert.equal(inventory.prerequisites[0].executable.path, npm);
    assert.deepEqual(inventory.prerequisites[0].args, ["run", "test:types"]);
    assert(inventory.prerequisites[0].timeoutMs <= receipt.boundsMs.prerequisiteCeiling);
    receipt.additionalRequiredCoverage = inventory.additionalRequiredCoverage;
    sourceBefore = snapshotInputs(original);
    originalBefore = snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] });
    projectBefore = dependencyTree(join(original, "node_modules"));
    npmBefore = dependencyTree(npmRoot);
    assert.deepEqual(sourceBefore, inventory.sourceSnapshot);
    assert.equal(projectBefore.sha256, inventory.dependencySnapshot.manifestSha256);
    assert.equal(npmBefore.sha256, inventory.npmRuntimeSnapshot.manifestSha256);
    for (const [name, snapshot] of [["source", sourceBefore], ["original", originalBefore], ["project-dependencies", projectBefore], ["npm-runtime", npmBefore]]) save(`${name}-before.json`, snapshot);
    mkdirSync(join(output, "source"));
    for (const file of sourceBefore.files) {
      const destination = join(output, "source", file.path);
      mkdirSync(dirname(destination), { recursive: true });
      copyFileSync(join(original, file.path), destination);
    }
    for (const [path, name] of [[workloadPath, "maintained-workloads.json"], [inventoryPath, "required-tests.json"]]) copyFileSync(path, join(output, name));
    const result = await run("portgate", node, [join(root, "finer/tools/portgate.mjs"), "record", "--arm", recipe.armLabel, "--ports", recipe.workload, "--compiler", compiler, "--codec", codec, "--manifest", workloadPath, "--out", arm, "--build-timeout", "300", "--test-timeout", "90", "--codec-timeout", "60"], root, 600_000);
    assert(completed(result) && [0, 1].includes(result.status), "portgate did not complete; partial evidence retained");
    const manifest = json(join(arm, "manifest.json")), built = json(join(arm, `${recipe.workload}.json`));
    assert.deepEqual(manifest.ports, [recipe.workload]);
    assert.deepEqual(manifest.timeoutsMs, { build: 300_000, test: 90_000, codec: 60_000 });
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
    for (const [config, file] of [["lilscript.toml", `${recipe.artifactStem}.raw.js`], ["lilscript.closed.toml", `${recipe.artifactStem}.closed.js`]]) {
      const invocation = built.build.invocations.find(row => row.output.path === join(workspace, "dist", file));
      assert(invocation, `missing source-built output: ${file}`);
      assert.deepEqual(invocation.args, [join(workspace, "src/entry.lil"), "--target", "js-module", "--config", join(workspace, config), "-o", join(workspace, "dist", file)]);
      assert.equal(invocation.output.previous, null);
      assert.deepEqual(invocation.inputs, sourceBefore.files);
      assert.equal(invocation.configuration.sha256, fileIdentity(join(original, config)).sha256);
    }
    const expectedArtifacts = ["cjs", "closed.js", "esm.js", "raw.js", "umd.js"].map(suffix => `dist/${recipe.artifactStem}.${suffix}`).sort();
    assert.deepEqual(built.artifacts.map(row => row.path).sort(), expectedArtifacts);
    for (const artifact of built.artifacts) assert.deepEqual(fileIdentity(join(workspace, artifact.path)), { sha256: artifact.sha256, bytes: artifact.raw });
    assert.deepEqual(fileIdentity(join(workspace, `dist/${recipe.artifactStem}.d.ts`)), fileIdentity(join(original, `types/${recipe.artifactStem}.d.ts`)));
    const raw = readFileSync(join(workspace, `dist/${recipe.artifactStem}.raw.js`), "utf8");
    assert.equal(readFileSync(join(workspace, `dist/${recipe.artifactStem}.esm.js`), "utf8"), `${recipe.esmBanner}${raw.trimEnd()}\n`);
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
    assert.equal(report.evidence.cases.length, recipe.caseCount);
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
    assert.deepEqual(fileIdentity(join(workspace, `dist/${recipe.artifactStem}.d.ts`)), fileIdentity(join(original, `types/${recipe.artifactStem}.d.ts`)));
    const paths = built.artifacts.map(row => join(workspace, row.path));
    const measured = await run("canonical-codec", codec, ["--json", ...paths], workspace, 60_000);
    assert(completed(measured) && measured.status === 0, "canonical codec replay did not pass");
    const sizes = validateMeasurements(JSON.parse(measured.stdout), paths);
    for (const artifact of built.artifacts) for (const key of ["raw", "gzip9", "brotli11"]) assert.equal(sizes.get(join(workspace, artifact.path))[key], artifact[key]);
    receipt.passed = true;
    receipt.outcome = "Original source-built npm-test boundary and separate original package dry-run passed; additional delivery and browser/site requirements remain unverified.";
  } catch (error) {
    receipt.failure = { message: error.message, stack: error.stack };
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
    }
    try {
      if (existsSync(arm)) {
        for (const name of ["manifest.json", `${recipe.workload}.json`, "logs", "wrappers", "invocations", "tests"]) if (existsSync(join(arm, name))) cpSync(join(arm, name), join(output, name), { recursive: true });
        if (existsSync(join(workspace, "dist"))) cpSync(join(workspace, "dist"), join(output, "artifacts"), { recursive: true });
      }
    } catch (error) {
      receipt.passed = receipt.originalTestBoundaryPassed = receipt.packageDryRunPassed = false;
      receipt.archiveFailure = { message: error.message, stack: error.stack };
    }
    receipt.elapsedMs = elapsed();
    receipt.loadAverageEnd = loadavg();
    receipt.completed = new Date().toISOString();
    try { receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files; }
    catch (error) {
      receipt.passed = receipt.originalTestBoundaryPassed = receipt.packageDryRunPassed = false;
      receipt.outputManifestFailure = { message: error.message, stack: error.stack };
    }
    save("receipt.json", receipt);
  }
  return receipt;
}
