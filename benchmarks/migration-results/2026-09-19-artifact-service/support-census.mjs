import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
const compiler = "/tmp/lilscript-artifact-service-baseline-20260919/lilscript";
const qualification = JSON.parse(readFileSync(join(directory, "run-2026-09-19T13-05-45.258Z/receipt.json")));
const hash = value => createHash("sha256").update(value).digest("hex");
const file = path => ({ path, sha256: hash(readFileSync(path)), bytes: readFileSync(path).length });
assert(qualification.passed && qualification.inputsStable);
assert.equal(file(compiler).sha256, qualification.binaries.find(row => row.path.endsWith("/lilscript")).sha256);
const output = join(directory, `support-${new Date().toISOString().replaceAll(":", "-")}`);
mkdirSync(output);
const config = join(root, "tests/config/no-optimization.toml");
const env = { ...process.env, PATH: "/home/azureuser/.nvm/versions/node/v24.11.1/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" };
const receipt = {
  schema: 1, scope: "existing tests/cases runtime corpus, not a complete language feature inventory or source-built library baseline",
  backend: "semantic", mode: "development", compiler: file(compiler), compilerInputSha256: qualification.inputSha256,
  harness: file(fileURLToPath(import.meta.url)), config: file(config),
  node: spawnSync("node", ["--version"], { env, encoding: "utf8" }).stdout.trim(),
  cc: spawnSync("/usr/bin/cc", ["--version"], { env, encoding: "utf8" }).stdout.trim(),
  rows: [],
};
function run(name, command, args) {
  const start = performance.now();
  const result = spawnSync(command, args, { env, cwd: root, encoding: "utf8", timeout: 30_000, maxBuffer: 16 * 1024 * 1024 });
  writeFileSync(join(output, `${name}.stdout`), result.stdout ?? "");
  writeFileSync(join(output, `${name}.stderr`), result.stderr ?? "");
  return { command, args, status: result.status, signal: result.signal, error: result.error?.message, ms: performance.now() - start, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}
const sources = readdirSync(join(root, "tests/cases")).filter(name => name.endsWith(".lil")).sort();
for (const name of sources) {
  const source = join(root, "tests/cases", name);
  const stem = name.slice(0, -4);
  const expectedPath = source.replace(/\.lil$/, ".out");
  const expected = readFileSync(expectedPath, "utf8");
  for (const target of ["js", "c"]) {
    const label = `${stem}-${target}`;
    const destination = join(output, `${label}.${target}`);
    const compile = run(`${label}-compile`, compiler, [source, "--config", config, "--backend", "semantic", "--mode", "development", "--target", target, "--output", destination, "--explain", "json"]);
    const row = { source: file(source), expected: file(expectedPath), target, compile, state: "unsupported-or-compile-failure", gapOwner: "007" };
    if (compile.status === 0) {
      const report = JSON.parse(compile.stderr);
      row.artifact = file(destination);
      row.compilerReport = report;
      const reportedHash = target === "c" ? report.native_sha256 : report.artifacts[report.winners[2]].sha256;
      assert.equal(row.artifact.sha256, reportedHash, `${label}: delivery hash`);
      let executable = destination;
      if (target === "c") {
        executable = join(output, `${label}.exe`);
        row.nativeBuild = run(`${label}-cc`, "/usr/bin/cc", ["-std=c11", "-O1", "-fno-fast-math", "-ffp-contract=off", destination, "-lm", "-o", executable]);
      }
      if (target === "js" || row.nativeBuild.status === 0) {
        row.execution = run(`${label}-execute`, target === "js" ? "node" : executable, target === "js" ? [destination] : []);
        row.state = row.execution.status === 0 && row.execution.stdout === expected ? "passed-original-observations" : "observation-mismatch-or-execution-failure";
        if (row.state === "passed-original-observations") delete row.gapOwner;
      } else row.state = "native-build-failure";
    }
    receipt.rows.push(row);
    writeFileSync(join(output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
    console.log(`${name} ${target}: ${row.state}`);
  }
}
receipt.summary = Object.fromEntries(["js", "c"].map(target => [target, receipt.rows.filter(row => row.target === target).reduce((counts, row) => ({ ...counts, [row.state]: (counts[row.state] ?? 0) + 1 }), {})]));
receipt.corpusFiles = sources.length;
receipt.completed = new Date().toISOString();
receipt.limits = ["Historical qualified compiler pin, not later in-progress source", "Only existing script observations, not complete public adapters or every language form", "Host-specific versus portable exclusions still need classification", "No release-speed/RSS or competitive-size claims"];
writeFileSync(join(output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify({ directory: output, summary: receipt.summary }));
