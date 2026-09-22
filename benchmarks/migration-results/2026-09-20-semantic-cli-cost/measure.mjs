import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { cpus, platform, release } from "node:os";
import { fileURLToPath } from "node:url";
import { digest, fileIdentity, snapshotInputs } from "../../../finer/tools/artifact-evidence.mjs";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
assert(process.argv.slice(2).every(arg => arg === "--sweep"));
const sweep = process.argv.includes("--sweep");
const root = resolve(directory, "../../..");
const parent = join(root, "benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-20T02-12-30.329Z");
const fixture = join(root, "src/semantic_program/fixtures/integrated-architecture");
const saved = "/tmp/lilscript-semantic-cli-cost-20260920";
const output = join(directory, `run-${new Date().toISOString().replaceAll(":", "-")}`);
mkdirSync(output);
mkdirSync(saved, { recursive: true });
const read = path => JSON.parse(readFileSync(path));
const identity = path => ({ path: relative(root, path), ...fileIdentity(path) });
const save = (name, value) => writeFileSync(join(output, name), JSON.stringify(value, null, 2) + "\n");
const qualification = read(join(parent, "receipt.json"));
const inputs = read(join(parent, "inputs-before.json"));
const receipt = {
  schema: 1, started: new Date().toISOString(), passed: false,
  scope: "production CLI, unchanged 12-module integration source/host fixture; timing observation or explicit configuration ablation, not a speed improvement or whole-library qualification",
  sweep,
  protocol: { pairs: 5, warmup: 1, process: "fresh per compile", order: "alternating off/on and on/off pairs", cache: "warm OS, no compiler session reuse", timeoutMs: 60000, rayonThreads: 1, objective: "original Brotli-only config", externalQualification: "after all measured compiles", deadlines: "no wall-clock compiler limit", timeScope: "GNU time covers the whole CLI including configuration, explanation and file delivery" },
  qualification: identity(join(parent, "receipt.json")), runner: identity(fileURLToPath(import.meta.url)),
  host: { platform: platform(), release: release(), cpu: cpus()[0]?.model, logicalCpus: cpus().length, node: process.version },
  commands: [], samples: [], binaries: [],
};
const baseEnv = { ...process.env, RAYON_NUM_THREADS: "1", LC_ALL: "C" };
delete baseEnv.LILSCRIPT_TIMING;
async function command(label, executable, args, env = baseEnv) {
  const start = performance.now();
  const result = await runBoundedCommand(executable, args, { cwd: root, env, encoding: "utf8", timeoutMs: 60000, maxBuffer: 16 * 1024 * 1024 });
  writeFileSync(join(output, `${label}.stdout`), result.stdout);
  writeFileSync(join(output, `${label}.stderr`), result.stderr);
  receipt.commands.push({ label, executable, args, timingEnabled: env.LILSCRIPT_TIMING === "1", status: result.status, signal: result.signal, error: result.error?.message, supervision: result.supervision, wallMs: performance.now() - start });
  save("receipt.json", receipt);
  assert.equal(result.status, 0, `${label}: ${result.stderr}`);
  return result;
}
function checkInputs() {
  assert.equal(digest(JSON.stringify(inputs.files)), inputs.sha256);
  for (const entry of inputs.files) assert.deepEqual(identity(join(root, entry.path)), entry, entry.path);
  return { count: inputs.files.length, sha256: inputs.sha256 };
}
const stableReport = report => {
  const { first_artifact_ns, phases_ns, total_ns, ...stable } = report;
  return stable;
};
const stats = values => {
  const sorted = values.toSorted((a, b) => a - b);
  return { min: sorted[0], median: sorted[Math.floor(sorted.length / 2)], max: sorted.at(-1) };
};
try {
  assert.equal(qualification.passed, true);
  assert.equal(qualification.inputsStable, true);
  assert.equal(qualification.inputSha256, inputs.sha256);
  receipt.inputsBefore = checkInputs();
  receipt.fixtureBefore = snapshotInputs(fixture);
  for (const [index, entry] of [...qualification.binaries, ...qualification.testBinaries].entries()) {
    const source = join(root, entry.path);
    assert.deepEqual(identity(source), entry);
    const path = join(saved, ["lilscript", "lilscript-codec", "semantic-integrated", "library-tests", "cli-tests"][index]);
    copyFileSync(source, path);
    assert.deepEqual(fileIdentity(path), { sha256: entry.sha256, bytes: entry.bytes });
    receipt.binaries.push({ source: entry, preserved: identity(path) });
  }
  const compiler = join(saved, "lilscript"), codec = join(saved, "lilscript-codec");
  const args = [join(fixture, "entry.lil"), "--config", join(fixture, "config.toml"), "--backend", "semantic", "--target", "js-module", "--explain", "json"];
  const observations = [];
  let schedule = [{ label: "warmup", enabled: false, measured: false }];
  if (sweep) {
    receipt.protocol = { ...receipt.protocol, pairs: null, rounds: 3, warmup: 1, order: "ten configurations, forward/reverse/forward", objective: "Brotli only; probe limit and codec schedule varied explicitly, all other resolved policy fields checked equal" };
    const parser = "import json,sys,tomllib; print(json.dumps([tomllib.load(open(p,'rb')) for p in sys.argv[1:]]))";
    const original = JSON.parse((await command("original-config-parse", "python3", ["-c", parser, join(fixture, "config.toml")])).stdout)[0];
    assert.deepEqual(original, { javascript: { optimization_level: 13, priority: "size-first", cost_model: "brotli", strip_console: false, ecmascript: "es2022", assume_pristine_builtins: false, candidate_proposal_limit: 96, terminal_codec_probe_limit: 192 } });
    const configurations = [];
    for (const probes of [8, 24, 48, 96, 192]) for (const codecSchedule of ["immediate", "staged"]) {
      const label = `${codecSchedule}-${probes}`, path = join(output, `${label}.toml`);
      // A closed scalar fixture schema, checked with tomllib below, not a general TOML writer.
      const javascript = { ...original.javascript, terminal_codec_probe_limit: probes };
      writeFileSync(path, "[javascript]\n" + Object.entries(javascript).map(([key, value]) => `${key} = ${JSON.stringify(value)}\n`).join("") + `\n[policy.search]\ncodec_schedule = ${JSON.stringify(codecSchedule)}\n`);
      configurations.push({ label, path, probes, codecSchedule, expected: { javascript, policy: { search: { codec_schedule: codecSchedule } } } });
    }
    const parsed = JSON.parse((await command("variant-config-parse", "python3", ["-c", parser, ...configurations.map(config => config.path)])).stdout);
    assert.deepEqual(parsed, configurations.map(config => config.expected));
    receipt.configurations = configurations.map(config => ({ ...config, file: identity(config.path) }));
    for (let round = 0; round < 3; round++) for (const config of round % 2 ? configurations.toReversed() : configurations) {
      schedule.push({ label: `${round}-${config.label}`, group: config.label, config: config.path, probes: config.probes, codecSchedule: config.codecSchedule, round, enabled: true, measured: true });
    }
  } else {
    for (let pair = 0; pair < 5; pair++) for (const enabled of pair % 2 ? [true, false] : [false, true]) {
      schedule.push({ label: `pair-${pair}-${enabled ? "on" : "off"}`, enabled, measured: true, pair });
    }
  }
  for (const sample of schedule) {
    const artifact = join(output, `${sample.label}.mjs`), time = join(output, `${sample.label}.time`);
    const env = { ...baseEnv, ...(sample.enabled ? { LILSCRIPT_TIMING: "1" } : {}) };
    const compileArgs = args.slice();
    if (sample.config) compileArgs[2] = sample.config;
    const result = await command(sample.label, "/usr/bin/time", ["-f", '{"wallSeconds":%e,"userSeconds":%U,"systemSeconds":%S,"maxRssKiB":%M}', "-o", time, compiler, ...compileArgs, "--output", artifact], env);
    assert.equal(result.stdout, "");
    const lines = result.stderr.split("\n");
    const telemetry = lines.filter(line => line.startsWith("lilscript-timing "));
    assert.equal(telemetry.length, sample.enabled ? 1 : 0);
    const report = JSON.parse(lines.filter(line => !line.startsWith("lilscript-timing ")).join("\n"));
    const entry = { ...sample, artifact: identity(artifact), report, process: read(time), timing: telemetry.length ? JSON.parse(telemetry[0].slice("lilscript-timing ".length)) : null };
    assert.equal(report.shape.modules, 12);
    assert.equal(report.resources.retained_bytes_after_handoff, 0);
    assert.equal(report.ledger_after_finish.retained_bytes, 0);
    const previous = observations.find(other => other.group === sample.group);
    if (previous) {
      assert.deepEqual(stableReport(report), stableReport(previous.report));
      assert.equal(entry.artifact.sha256, previous.artifact.sha256);
    }
    if (sample.config) {
      const policy = structuredClone(report.javascript_policy);
      assert.equal(policy.objective.optional_codec_probes, sample.probes);
      assert.equal(policy.objective.search.codec_schedule, sample.codecSchedule);
      policy.objective.optional_codec_probes = 192;
      policy.objective.search.codec_schedule = "staged";
      assert.deepEqual(policy, observations[0].report.javascript_policy);
      assert.deepEqual(report.inputs, observations[0].report.inputs);
      assert.deepEqual(report.request, observations[0].report.request);
      assert.deepEqual(report.search.request, observations[0].report.search.request);
    }
    observations.push(entry);
    receipt.samples = observations;
    save("receipt.json", receipt);
    console.log(`${sample.label}: ${report.total_ns / 1e6} ms service, ${entry.process.maxRssKiB} KiB RSS`);
  }
  const setup = readFileSync(join(fixture, "setup.js"), "utf8");
  const host = readFileSync(join(fixture, "host.js"), "utf8");
  const observer = `import {pathToFileURL} from 'node:url';\nconst events=[];\n${setup}\nconst library=await import(pathToFileURL(process.argv[1]).href);\n${host}\nprocess.stdout.write(JSON.stringify(events));`;
  const expected = read(join(fixture, "expected.json"));
  for (const sample of observations) {
    const artifact = join(root, sample.artifact.path);
    const observed = await command(`${sample.label}-observe`, process.execPath, ["--input-type=module", "-e", observer, artifact]);
    assert.deepEqual(JSON.parse(observed.stdout), expected);
    const rescored = await command(`${sample.label}-rescore`, codec, ["--json", artifact]);
    const rescoredReport = JSON.parse(rescored.stdout);
    assert.equal(rescoredReport.artifacts.length, 1);
    const sizes = rescoredReport.artifacts[0];
    const winner = sample.report.artifacts[sample.report.winners[2]];
    assert.equal(winner.sha256, sample.artifact.sha256);
    assert.equal(winner.raw, sizes.raw);
    assert.equal(winner.brotli11, sizes.brotli11);
    assert.equal(winner.gzip9, null);
    sample.canonicalSizes = sizes;
    sample.observationsPassed = true;
  }
  const measured = observations.filter(sample => sample.measured);
  const summary = { byTiming: {}, telemetry: {}, phasesMs: {}, equalStableReportsAndBytes: true, samples: measured.length };
  for (const enabled of [false, true]) {
    const samples = measured.filter(sample => sample.enabled === enabled);
    if (!samples.length) continue;
    summary.byTiming[enabled ? "on" : "off"] = {
      serviceMs: stats(samples.map(sample => sample.report.total_ns / 1e6)),
      firstArtifactMs: stats(samples.map(sample => sample.report.first_artifact_ns / 1e6)),
      ...Object.fromEntries(Object.keys(samples[0].process).map(key => [key, stats(samples.map(sample => sample.process[key]))])),
    };
  }
  const enabled = measured.filter(sample => sample.enabled);
  for (const key of Object.keys(enabled[0].timing)) summary.telemetry[key] = stats(enabled.map(sample => sample.timing[key]));
  for (const key of Object.keys(enabled[0].report.phases_ns)) summary.phasesMs[key] = stats(enabled.map(sample => sample.report.phases_ns[key] / 1e6));
  if (sweep) {
    // Cross-configuration medians would conflate different amounts of work.
    delete summary.byTiming;
    delete summary.telemetry;
    delete summary.phasesMs;
    summary.equalStableReportsAndBytes = "within each configuration only";
    summary.rows = receipt.configurations.map(config => {
      const samples = measured.filter(sample => sample.group === config.label), first = samples[0];
      return {
        config: config.label, samples: samples.length, sizes: first.canonicalSizes,
        serviceMs: stats(samples.map(sample => sample.report.total_ns / 1e6)),
        brotliMs: stats(samples.map(sample => sample.timing.canonical_brotli_ms)),
        rssKiB: stats(samples.map(sample => sample.process.maxRssKiB)),
        search: first.report.search, resources: first.report.resources,
      };
    });
  }
  save("summary.json", summary);
  receipt.summary = identity(join(output, "summary.json"));
  receipt.inputsAfter = checkInputs();
  receipt.fixtureAfter = snapshotInputs(fixture);
  assert.deepEqual(receipt.fixtureAfter, receipt.fixtureBefore);
  for (const binary of receipt.binaries) assert.deepEqual(identity(join(root, binary.preserved.path)), binary.preserved);
  receipt.passed = true;
  console.log(JSON.stringify({ output, summary }, null, 2));
} catch (error) {
  receipt.error = error.stack;
  process.exitCode = 1;
  console.error(error);
} finally {
  receipt.completed = new Date().toISOString();
  save("receipt.json", receipt);
}
