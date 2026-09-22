import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { loadavg } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
assert.equal(process.argv.length, 3, "usage: qualify-retained-advance.mjs new-output-directory (relative to this script directory)");
const output = resolve(directory, process.argv[2]);
mkdirSync(output, { recursive: false });
const accepted = join(directory, "run-2026-09-19T15-55-00.967Z");
const preserved = "/tmp/lilscript-parse-once-baseline-20260919";
const driver = join(preserved, "semantic-integrated");
const codec = join(preserved, "lilscript-codec");
const runner = join(root, "finer/tools/semantic-integration.py");
const helper = join(root, "finer/tools/bounded-command.mjs");
const fixture = join(root, "src/semantic_program/fixtures/integrated-architecture");
const python = "/usr/bin/python3";
const node = "/home/azureuser/.nvm/versions/node/v24.11.1/bin/node";
const nativeCc = "/usr/bin/cc";
const nativeClang = "/tmp/lilscript-native-core-20260913/toolchain/root/usr/bin/clang-18";
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const identity = path => ({ path: relative(root, path), sha256: hash(readFileSync(path)), bytes: statSync(path).size });
const json = path => JSON.parse(readFileSync(path, "utf8"));
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
function filesUnder(path) {
  return readdirSync(path, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))
    .flatMap(entry => entry.isDirectory() ? filesUnder(join(path, entry.name)) : [join(path, entry.name)]);
}
function inputs() {
  const files = [fileURLToPath(import.meta.url), runner, helper, join(root, ".nvmrc"),
    join(accepted, "receipt.json"), join(accepted, "inputs-before.json"), join(accepted, "inputs-after.json"),
    driver, codec, node, python, "/usr/bin/time", nativeCc, nativeClang, ...filesUnder(fixture)]
    .map(identity).sort((a, b) => a.path.localeCompare(b.path));
  return { sha256: hash(JSON.stringify(files)), files };
}
const receipt = {
  schema: 1, started: new Date().toISOString(), passed: false,
  scope: "five repeated debug retained-history versus advancing-current ownership pairs, one unchanged integrated fixture",
  controls: { pairs: 5, edits: 16, retain_at: 1, work: 200_000_000, memory: 256_000_000 },
  node: { version: process.version, executable: process.execPath },
  load_average_start: loadavg(),
  limitations: [
    "Both arms use the same preserved debug compiler, original runner and fixture; no compiler or library changes are made.",
    "This compares retained versus advancing persistent ownership, not incremental compilation versus fresh recomputation.",
    "Edit duration sums edit plus parent-release time after the common first fork; initial compilation, rendering, codec replay and host observation are outside that interval.",
    "The retained arm keeps edit history; the advancing arm still retains the baseline and the branch after edit one.",
    "Copied bytes are reported semantic payload copies, not total allocations, process memory, or operating-system copy volume.",
    "GNU time peak RSS and process timings cover the whole driver invocation, not only edits or external artifact qualification.",
    "Debug timings on a shared, non-isolated host with uncontrolled background load are diagnostic; no release-speed, confidence interval or speed-win claim.",
    "All five pairs and their original qualification outputs are retained; no samples are excluded or retried.",
    "The enclosing 600-second POSIX process-group timeout bounds the existing runner, whose inner commands remain synchronous.",
  ],
};
let initial;
try {
  initial = inputs();
  receipt.inputs = initial;
  save("inputs-before.json", initial);
  assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`);
  assert.equal(identity(process.execPath).sha256, identity(node).sha256, "wrapper must run with the pinned Node binary");
  const qualification = json(join(accepted, "receipt.json"));
  assert.equal(qualification.passed, true);
  assert.equal(qualification.inputsStable, true);
  const manifests = ["inputs-before.json", "inputs-after.json"].map(name => {
    const manifest = json(join(accepted, name));
    assert.equal(hash(JSON.stringify(manifest.files)), manifest.sha256);
    assert.equal(manifest.sha256, qualification.inputSha256);
    copyFileSync(join(accepted, name), join(output, `qualified-${name}`));
    return manifest;
  });
  assert.deepEqual(manifests[0].files, manifests[1].files);
  const sourceFiles = manifests[0].files;
  for (const path of [runner, helper, join(root, ".nvmrc"), ...filesUnder(fixture)]) {
    const actual = identity(path);
    assert.deepEqual(sourceFiles.find(file => file.path === actual.path), actual, `input differs from accepted compiler qualification: ${path}`);
  }
  for (const path of [driver, codec]) {
    const candidates = qualification.binaries.filter(file => basename(file.path) === basename(path));
    assert.equal(candidates.length, 1);
    assert(candidates[0].path.includes("/debug/"));
    assert.equal(identity(path).sha256, candidates[0].sha256);
    assert.equal(identity(path).bytes, candidates[0].bytes);
  }
  receipt.qualification = { ...identity(join(accepted, "receipt.json")), compilerInputsSha256: qualification.inputSha256 };
  for (const [from, name] of [[fileURLToPath(import.meta.url), "wrapper.mjs"], [runner, "semantic-integration.py"], [helper, "bounded-command.mjs"], [join(accepted, "receipt.json"), "qualified-receipt.json"]]) {
    copyFileSync(from, join(output, name));
  }
  const overrides = {
    PATH: `${dirname(node)}:/home/azureuser/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`,
    RUST_BACKTRACE: "0", LILSCRIPT_NATIVE_CC: nativeCc, LILSCRIPT_NATIVE_CLANG: nativeClang, CC: nativeCc,
    PYTHONUNBUFFERED: "1",
  };
  receipt.environment = { cwd: root, overrides, other_variables: "inherited" };
  const commandArgs = [runner, "--driver", driver, "--codec", codec, "--fixture", fixture,
    "--output", join(output, "paired"), "--mode", "paired", "--pairs", "5", "--edits", "16", "--retain-at", "1"];
  receipt.command = { executable: python, args: commandArgs, timeoutMs: 600_000, killAfterMs: 1000, maxBuffer: 16 * 1024 * 1024 };
  save("receipt.json", receipt);
  const started = process.hrtime.bigint();
  const result = await runBoundedCommand(python, commandArgs, {
    cwd: root, env: { ...process.env, ...overrides }, timeoutMs: receipt.command.timeoutMs,
    killAfterMs: receipt.command.killAfterMs, maxBuffer: receipt.command.maxBuffer,
  });
  const wallNs = Number(process.hrtime.bigint() - started);
  writeFileSync(join(output, "runner.stdout"), result.stdout);
  writeFileSync(join(output, "runner.stderr"), result.stderr);
  receipt.execution = {
    wall_ns: wallNs, status: result.status, signal: result.signal,
    error: result.error ? { code: result.error.code, message: result.error.message } : null,
    supervision: result.supervision,
  };
  save("receipt.json", receipt);
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.status, 0, "original runner failed; all available outputs are preserved");
  assert.equal(result.signal, null);
  const summary = json(join(output, "paired/summary.json"));
  const contract = json(join(output, "paired/contract.json"));
  assert.equal(summary.complete, true);
  assert.equal(summary.qualified, true);
  assert.deepEqual(summary.issues, []);
  assert.equal(summary.runs.length, 10);
  assert.equal(summary.pairs.length, 5);
  assert.equal(contract.driver_sha256, identity(driver).sha256);
  assert.equal(contract.codec_sha256, identity(codec).sha256);
  assert.equal(contract.runner_sha256, identity(runner).sha256);
  for (const [name, value] of Object.entries({ mode: "paired", pairs: 5, edits: 16, retain_at: 1, work: 200_000_000, memory: 256_000_000 })) {
    assert.equal(contract.arguments[name], value);
  }
  const expectedLabels = ["baseline", "retained-branch", "final", "retained-branch-recheck", "baseline-recheck"];
  const reports = new Map();
  for (const [index, run] of summary.runs.entries()) {
    const ordinal = Math.floor(index / 2);
    const order = ordinal % 2 === 0 ? ["retain", "advance"] : ["advance", "retain"];
    assert.equal(run.ordinal, ordinal);
    assert.equal(run.mode, order[index % 2]);
    assert.equal(run.qualified, true);
    assert.deepEqual(run.issues, []);
    assert.equal(run.exit_code, 0);
    assert.equal(run.checks.length, 6, "each run must execute all five artifacts and independently replay codecs");
    assert(run.checks.every(check => check.exit_code === 0));
    const reportPath = join(run.directory, "report.json");
    assert.equal(identity(reportPath).sha256, run.report_sha256);
    const report = json(reportPath);
    assert.equal(report.edits.length, 16);
    assert.deepEqual(report.artifacts.map(artifact => artifact.label), expectedLabels);
    assert.deepEqual(run.artifacts, report.artifacts.map(artifact => [artifact.label, artifact.sha256]));
    for (const source of report.sources) {
      const actual = identity(source.path);
      assert.deepEqual(sourceFiles.find(file => file.path === actual.path), actual);
      assert.equal(source.sha256, actual.sha256);
      assert.equal(source.bytes, actual.bytes);
    }
    for (const artifact of report.artifacts) {
      const bytes = readFileSync(join(run.directory, artifact.file));
      assert.equal(hash(bytes), artifact.sha256);
      assert.equal(bytes.length, artifact.bytes);
    }
    reports.set(`${ordinal}-${run.mode}`, { report, directory: run.directory });
  }
  for (const [ordinal, pair] of summary.pairs.entries()) {
    assert.equal(pair.ordinal, ordinal);
    assert.equal(pair.qualified, true);
    const retain = reports.get(`${ordinal}-retain`);
    const advance = reports.get(`${ordinal}-advance`);
    assert.deepEqual(retain.report.sources, advance.report.sources);
    for (const label of expectedLabels) {
      assert(readFileSync(join(retain.directory, `${label}.mjs`)).equals(readFileSync(join(advance.directory, `${label}.mjs`))), `pair ${ordinal} differs: ${label}`);
    }
  }
  const sample = run => ({
    post_fork_edit_and_release_ns: run.edit_ns,
    copied_payload_bytes: run.copied_payload_bytes,
    post_fork_copied_payload_bytes: run.post_fork_copied_payload_bytes,
    reused_units: run.reused_units,
    post_fork_reused_units: run.post_fork_reused_units,
    whole_driver_peak_rss_kib: run.process.peak_rss_kib,
    whole_driver_wall_seconds: run.process.wall_s,
  });
  receipt.samples = Object.fromEntries(["retain", "advance"].map(mode => [mode, summary.runs.filter(run => run.mode === mode).map(sample)]));
  receipt.medians = Object.fromEntries(Object.entries(receipt.samples).map(([mode, samples]) => [mode,
    Object.fromEntries(Object.keys(samples[0]).map(key => {
      const values = samples.map(row => row[key]);
      assert(values.every(value => Number.isFinite(value) && value >= 0));
      return [key, values.sort((a, b) => a - b)[2]];
    })),
  ]));
  receipt.pairs = summary.pairs;
  receipt.edit_ratio_summary = summary.edit_ratio_summary;
  receipt.counts = { pairs: 5, driver_runs: 10, edits: 160, artifact_executions: 50, canonical_codec_runs: 10, canonical_artifact_replays: 50 };
  receipt.summary = identity(join(output, "paired/summary.json"));
  receipt.contract = identity(join(output, "paired/contract.json"));
  receipt.passed = true;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
  console.error(error.message);
} finally {
  try {
    const final = inputs();
    save("inputs-after.json", final);
    receipt.inputsStable = initial?.sha256 === final.sha256;
    if (!receipt.inputsStable) throw new Error("Pinned comparison inputs changed or were not captured before execution");
  } catch (error) {
    receipt.inputsStable = false;
    receipt.passed = false;
    receipt.failure ??= { message: error.message, stack: error.stack };
    process.exitCode = 1;
  }
  receipt.generatedFiles = filesUnder(output).filter(path => path !== join(output, "receipt.json")).map(identity);
  receipt.load_average_end = loadavg();
  receipt.completed = new Date().toISOString();
  save("receipt.json", receipt);
  console.log(JSON.stringify({ directory: output, passed: receipt.passed, inputsStable: receipt.inputsStable }));
}
