import assert from "node:assert/strict";
import { copyFileSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs } from "../../../finer/tools/artifact-evidence.mjs";
import { runNodeTestEvidence } from "../../../finer/tools/node-test-evidence.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
const original = "/home/azureuser/remark-rehypelil";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const npm = join(dirname(node), "npm"), npmRoot = dirname(dirname(realpathSync(npm)));
const output = join(here, "existing-dist");
mkdirSync(output, { recursive: false });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const identify = path => ({ path, ...fileIdentity(path) });
const local = path => ({ path, ...fileIdentity(join(original, path)) });
const scratch = mkdtempSync(join(tmpdir(), "lilscript-remark-rehype-discovery-20260919-"));
const userConfig = join(output, "npm-user.npmrc"), globalConfig = join(output, "npm-global.npmrc");
writeFileSync(userConfig, "");
writeFileSync(globalConfig, "");
const removed = Object.keys(process.env).filter(key => key.startsWith("LILSCRIPT_") || /^npm_/i.test(key) || ["NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
for (const key of removed) delete process.env[key];
const overrides = {
  PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "",
  npm_config_offline: "true", npm_config_ignore_scripts: "true", npm_config_audit: "false", npm_config_fund: "false",
  npm_config_userconfig: userConfig, npm_config_globalconfig: globalConfig, npm_config_cache: join(scratch, "npm-cache"),
};
Object.assign(process.env, overrides);
const toolPaths = ["artifact-evidence", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts"].map(name => join(root, `finer/tools/${name}.mjs`));
const pinPaths = [fileURLToPath(import.meta.url), node, npm, join(root, ".nvmrc"), userConfig, globalConfig, ...toolPaths];
const pins = () => { const files = pinPaths.map(identify); return { files, sha256: fingerprint(files) }; };

// Same bounded complete-tree capture as the source-built Remark/Marked receipts.
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

const files = ["test/api.test.mjs", "test/closed.test.mjs", "test/site.test.mjs", "test/official/test.js"];
const fixtures = [
  "package.json", "package-lock.json", "tsconfig.json", "types/remark-rehype.d.ts", "test/types.test.ts",
  "scripts/check-pack.mjs", "scripts/build-site.mjs", "site/index.html", "site/app.js", "site/results.json",
].map(local);
const prerequisites = [{
  id: "original-test-types", executable: identify(npm), args: ["run", "test:types"], timeoutMs: 60_000,
  inputs: ["package.json", "package-lock.json", "tsconfig.json", "types/remark-rehype.d.ts", "dist/remark-rehype.d.ts", "test/types.test.ts", "node_modules/typescript/package.json", "node_modules/typescript/bin/tsc", "node_modules/typescript/lib/tsc.js"].map(local),
}];
const started = process.hrtime.bigint();
const elapsedMs = () => Number(process.hrtime.bigint() - started) / 1e6;
const receipt = {
  schema: 1, kind: "existing-dist-remark-rehype-inventory-discovery", started: new Date().toISOString(),
  passed: false, qualification: "unverified", sourceBuilt: false,
  boundsMs: { externalSupervisor: 90_000, sharedAttempt: 90_000, typePrerequisite: 60_000, nodeSuiteCeiling: 90_000 },
  environment: { removed, overrides, otherwise: "inherited; original workspace .npmrc preserved if present" },
  limitations: [
    "Existing distribution only; no compiler execution, source-built qualification, canonical size replay or new runtime assertions.",
    "The original upstream package supplies only numeric footnote helper reference values; candidate plugin behavior uses generated ESM, not an upstream plugin substitution.",
    "Original Node selection exercises ESM behavior; its closed test only imports the artifact and checks callable-export shape. It does not execute CJS/UMD, install the package, rebuild the site or exercise a browser.",
    "Project and global npm dependency contents are pinned, not a hermetic OS environment. The outer bound covers sequential prerequisite and suite commands together.",
  ],
};
let before;
save("receipt.json", receipt);
try {
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  before = {
    inputs: pins(), source: snapshotInputs(original),
    original: snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] }),
    project: dependencyTree(join(original, "node_modules")), npm: dependencyTree(npmRoot),
  };
  save("before.json", before);
  save("declarations.json", { files, fixtures, prerequisites });
  copyFileSync(fileURLToPath(import.meta.url), join(output, "discovery-source.mjs"));
  mkdirSync(join(output, "runner-sources"));
  for (const path of toolPaths) copyFileSync(path, join(output, "runner-sources", basename(path)));
  const timeoutMs = Math.floor(90_000 - elapsedMs());
  assert(timeoutMs > 0, "discovery budget exhausted before Node owner");
  const report = await runNodeTestEvidence({
    cwd: original, files, requiredFilePatterns: ["test/*.test.mjs", "test/official/test.js"],
    artifactPaths: ["dist/remark-rehype.esm.js", "dist/remark-rehype.closed.js"],
    requiredCases: [], requiredTestFiles: files.map(local), requiredFixtures: fixtures,
    prerequisites, directory: join(output, "node"), timeoutMs,
  });
  receipt.report = identify(join(output, "node/report.json"));
  receipt.suiteExecuted = report.suiteExecuted;
  receipt.observedCases = report.evidence.cases;
  receipt.prerequisites = report.prerequisites;
  receipt.discoveryErrors = report.errors;
  receipt.passed = report.suiteExecuted && report.evidence.exitCode === 0 && report.evidence.cases.length > 0 && report.evidence.cases.every(row => row.status === "pass") && report.prerequisites.every(row => row.attempted && row.status === 0 && !row.signal && !row.error) && report.errors.length === 1 && report.errors[0] === "required test inventory missing or ambiguous";
  if (!receipt.passed) process.exitCode = 1;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  try {
    const after = {
      inputs: pins(), source: snapshotInputs(original),
      original: snapshotInputs(original, { exclude: [".git", "node_modules", "target", ".cache"] }),
      project: dependencyTree(join(original, "node_modules")), npm: dependencyTree(npmRoot),
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
