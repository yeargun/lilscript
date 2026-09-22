import assert from "node:assert/strict";
import { copyFileSync, cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fileIdentity, fingerprint, snapshotInputs, snapshotDependencyTree } from "../../../finer/tools/artifact-evidence.mjs";
import { runNodeTestEvidence } from "../../../finer/tools/node-test-evidence.mjs";

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, "../../..");
const original = "/home/azureuser/remark-gfmlil", node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const npm = join(dirname(node), "npm"), npmRoot = dirname(dirname(realpathSync(npm)));
const output = join(here, "existing-dist"), scratch = mkdtempSync("/tmp/lilscript-remark-gfm-discovery-20260920-");
const workspace = join(scratch, "remark-gfmlil");
mkdirSync(output);
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const identify = path => ({ path, ...fileIdentity(path) });
const local = path => ({ path, ...fileIdentity(join(original, path)) });
const userConfig = join(output, "npm-user.npmrc"), globalConfig = join(output, "npm-global.npmrc");
writeFileSync(userConfig, "");
writeFileSync(globalConfig, "");
const removed = Object.keys(process.env).filter(key => key.startsWith("LILSCRIPT_") || /^npm_/i.test(key) || ["UPDATE", "NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key));
for (const key of removed) delete process.env[key];
const overrides = {
  PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "",
  npm_config_offline: "true", npm_config_ignore_scripts: "true", npm_config_audit: "false", npm_config_fund: "false",
  npm_config_userconfig: userConfig, npm_config_globalconfig: globalConfig, npm_config_cache: join(scratch, "npm-cache"),
};
Object.assign(process.env, overrides);
const tools = ["artifact-evidence", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts"].map(name => join(root, `finer/tools/${name}.mjs`));
const pins = () => { const files = [fileURLToPath(import.meta.url), node, npm, join(root, ".nvmrc"), userConfig, globalConfig, ...tools].map(identify); return { files, sha256: fingerprint(files) }; };
const full = directory => snapshotInputs(directory, { exclude: [".git", "node_modules", "target", ".cache"] });
const capture = () => ({ inputs: pins(), source: snapshotInputs(original), original: full(original), workspace: full(workspace), project: snapshotDependencyTree(join(original, "node_modules")), npm: snapshotDependencyTree(npmRoot) });
const files = ["test/api.test.mjs", "test/closed.test.mjs", "test/site.test.mjs", "test/official/index.js"];
const folders = ["autolink-literal", "strikethrough-default", "strikethrough-not-one", "table", "table-no-align", "table-no-padding", "table-string-length", "tasklist"];
const prerequisites = [{
  id: "original-check-types", executable: identify(npm), args: ["run", "check:types"], timeoutMs: 60_000,
  inputs: ["package.json", "package-lock.json", "types/remark-gfm.d.ts", "dist/remark-gfm.d.ts", "test/types.test.ts", "node_modules/typescript/package.json", "node_modules/typescript/bin/tsc", "node_modules/typescript/lib/tsc.js"].map(local),
}];
const receipt = {
  schema: 1, kind: "existing-dist-remark-gfm-inventory-discovery", started: new Date().toISOString(),
  passed: false, qualification: "unverified", sourceBuilt: false, original, workspace,
  boundsMs: { externalSupervisor: 90_000, sharedAttempt: 90_000, typePrerequisite: 60_000, nodeSuiteCeiling: 90_000 },
  environment: { removed, overrides, updateAbsent: !("UPDATE" in process.env), otherwise: "inherited; original workspace npm configuration preserved" },
  limitations: [
    "Existing distribution and original assertions only; no compiler execution, source-built qualification, installation or codec measurement.",
    "All original files, including eight parseable tree.json expectations, are copied and pinned. The original fixture fallback for table-no-align uses input.md when output.md is absent; it is not changed.",
    "CJS coverage checks only export/function shape. Closed coverage checks registration. Original site assertions read retained files, not a rebuilt site or browser.",
    "Original parser/serializer integration uses upstream remark with the candidate GFM plugin; no upstream GFM plugin substitutes for the candidate.",
    "Two unsupported historical configuration fields and the source qualifier's different type-script declaration remain explicit source-build prerequisites.",
    "Installed project/global npm contents are pinned, not the entire OS environment. D2 and broader delivery/competitor obligations remain open."
  ],
};
const started = performance.now();
let before;
save("receipt.json", receipt);
try {
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node));
  assert.equal(JSON.parse(readFileSync(join(original, "package.json"))).scripts.test, "npm run build && npm run check:types && node --test test/*.test.mjs test/official/index.js");
  const fixtureRoot = join(original, "test/official/fixtures");
  assert.deepEqual(readdirSync(fixtureRoot).sort(), folders);
  receipt.expectedTrees = folders.map(folder => {
    const path = `test/official/fixtures/${folder}/tree.json`;
    assert.equal(JSON.parse(readFileSync(join(original, path))).type, "root");
    return local(path);
  });
  cpSync(original, workspace, { recursive: true, filter: path => ![".git", "node_modules", "target", ".cache"].includes(relative(original, path).split("/")[0]) });
  symlinkSync(join(original, "node_modules"), join(workspace, "node_modules"), "dir");
  before = capture();
  assert.deepEqual(before.workspace.files, before.original.files);
  save("before.json", before);
  const fixtures = before.source.files.filter(row => /^(?:test\/|site\/|scripts\/|types\/|package(?:-lock)?\.json$|lilscript(?:\.closed)?\.toml$)/.test(row.path)).map(({ path, sha256, bytes }) => ({ path, sha256, bytes }));
  save("declarations.json", { files, fixtures, prerequisites });
  copyFileSync(fileURLToPath(import.meta.url), join(output, "discovery-source.mjs"));
  mkdirSync(join(output, "runner-sources"));
  for (const path of tools) copyFileSync(path, join(output, "runner-sources", basename(path)));
  const timeoutMs = Math.floor(90_000 - (performance.now() - started));
  assert(timeoutMs > 0, "discovery budget exhausted before Node owner");
  const report = await runNodeTestEvidence({ cwd: workspace, files, requiredFilePatterns: ["test/*.test.mjs", "test/official/index.js"], artifactPaths: ["dist/remark-gfm.esm.js", "dist/remark-gfm.cjs", "dist/remark-gfm.closed.js"], requiredCases: [], requiredTestFiles: files.map(local), requiredFixtures: fixtures, prerequisites, directory: join(output, "node"), timeoutMs });
  receipt.report = identify(join(output, "node/report.json"));
  receipt.observedCases = report.evidence.cases;
  receipt.prerequisites = report.prerequisites;
  receipt.discoveryErrors = report.errors;
  receipt.passed = report.suiteExecuted && report.evidence.exitCode === 0 && report.evidence.cases.length > 0 && report.evidence.cases.every(row => row.status === "pass") && report.errors.length === 1 && report.errors[0] === "required test inventory missing or ambiguous";
} catch (error) { receipt.failure = { message: error.message, stack: error.stack }; }
finally {
  try {
    const after = capture(); save("after.json", after);
    assert(before, "initial snapshot incomplete"); assert.deepEqual(after, before, "discovery inputs changed"); receipt.inputsStable = true;
  } catch (error) { receipt.passed = false; receipt.inputsStable = false; receipt.validationFailure = { message: error.message, stack: error.stack }; }
  receipt.elapsedMs = performance.now() - started;
  receipt.completed = new Date().toISOString();
  receipt.outputs = snapshotInputs(output, { exclude: ["receipt.json"] }).files;
  save("receipt.json", receipt);
  console.log(JSON.stringify({ output, passed: receipt.passed, inputsStable: receipt.inputsStable, cases: receipt.observedCases?.length, failure: receipt.failure?.message }));
  if (!receipt.passed) process.exitCode = 1;
}
