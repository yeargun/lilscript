import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
const output = join(directory, `run-${new Date().toISOString().replaceAll(":", "-")}`);
mkdirSync(output, { recursive: true });
const hash = value => createHash("sha256").update(value).digest("hex");
const identity = path => ({ path: relative(root, path), sha256: hash(readFileSync(path)), bytes: statSync(path).size });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
function inputs() {
  const files = [];
  function walk(path) {
    if (statSync(path).isDirectory()) for (const name of readdirSync(path).sort()) walk(join(path, name));
    else if (/\.(mjs|js|json)$/.test(path)) files.push(identity(path));
  }
  walk(join(root, "finer/tools"));
  for (const path of [".nvmrc", "benchmarks/libraries/maintained-workloads.json", "benchmarks/libraries/markedlil.profiles.tests.json"]) files.push(identity(join(root, path)));
  files.push(identity(fileURLToPath(import.meta.url)));
  files.sort((a, b) => a.path.localeCompare(b.path));
  return { sha256: hash(JSON.stringify(files)), files };
}
const before = inputs();
save("inputs-before.json", before);
const receipt = {
  schema: 1, scope: "bounded command owner and focused Node/Vitest evidence tools; not a library/compiler qualification",
  started: new Date().toISOString(), inputSha256: before.sha256,
  runtime: { version: process.version, ...identity(process.execPath) },
  limitations: [
    "POSIX group cleanup cannot kill escaped sessions or execute after supervisor SIGKILL",
    "Node timers depend on a responsive event loop; kernel-stuck children may not acknowledge signals",
    "Output limits count buffered stream bytes, not process RSS or all allocator overhead",
    "No full library suite or compiler build; installed dependency closure is not qualified by this tools receipt",
    "Per-command deadlines do not bound synchronous workspace copying, hashing or Git metadata reads",
  ],
};
try {
  assert.equal(process.version, "v24.11.1");
  const args = ["--test", ...[
    "bounded-command", "node-test-evidence", "vitest-test-evidence", "artifact-evidence", "migration-inventory",
  ].map(name => `finer/tools/${name}.test.mjs`)];
  const start = performance.now();
  const result = await runBoundedCommand(process.execPath, args, {
    cwd: root, env: { ...process.env, PATH: `${dirname(process.execPath)}:${process.env.PATH}` },
    timeoutMs: 120000, encoding: "utf8", maxBuffer: 16 * 1024 * 1024,
  });
  writeFileSync(join(output, "tests.stdout"), result.stdout);
  writeFileSync(join(output, "tests.stderr"), result.stderr);
  receipt.command = { command: process.execPath, args, status: result.status, signal: result.signal, error: result.error?.message, supervision: result.supervision, elapsedMs: performance.now() - start };
  const summary = result.stdout.match(/(?:#|ℹ) tests (\d+)[\s\S]*?(?:#|ℹ) pass (\d+)[\s\S]*?(?:#|ℹ) fail (\d+)/);
  assert.equal(result.status, 0, result.stderr + "\n" + result.stdout.slice(-5000));
  assert(summary && Number(summary[1]) > 0 && summary[1] === summary[2] && summary[3] === "0");
  receipt.tests = { total: Number(summary[1]), passed: Number(summary[2]), failed: Number(summary[3]) };
  receipt.passed = true;
} catch (error) {
  receipt.passed = false;
  receipt.failure = error.stack;
  process.exitCode = 1;
} finally {
  const after = inputs();
  save("inputs-after.json", after);
  receipt.inputsStable = before.sha256 === after.sha256;
  if (!receipt.inputsStable) { receipt.passed = false; process.exitCode = 1; }
  receipt.completed = new Date().toISOString();
  save("receipt.json", receipt);
  console.log(JSON.stringify({ directory: output, passed: receipt.passed, tests: receipt.tests, inputsStable: receipt.inputsStable, failure: receipt.failure }));
}
