import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../..");
const args = process.argv.slice(2);
assert.equal(args.length, 3, "usage: compare-parse-once.mjs before-run after-run output-dir (paths relative to this script directory)");
const [beforeDirectory, afterDirectory, outputDirectory] = args.map(path => resolve(directory, path));
mkdirSync(outputDirectory, { recursive: false });

const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const identity = path => ({ path: relative(root, path), sha256: hash(readFileSync(path)), bytes: statSync(path).size });
const save = (name, value) => writeFileSync(join(outputDirectory, name), JSON.stringify(value, null, 2) + "\n");
const readJson = path => JSON.parse(readFileSync(path, "utf8"));
const repetitions = 5;
const fixtureDirectory = join(root, "src/semantic_program/fixtures/integrated-architecture");
const config = join(fixtureDirectory, "config.toml");
const workloads = [
  { name: "entry", path: join(fixtureDirectory, "entry.lil") },
  { name: "native-entry", path: join(fixtureDirectory, "native-entry.lil") },
  { name: "factory", path: join(fixtureDirectory, "factory/entry.lil") },
];
const pinnedPaths = [fileURLToPath(import.meta.url), join(root, "finer/tools/bounded-command.mjs"), join(root, ".nvmrc"), process.execPath];
function fixtureFiles(path) {
  for (const entry of readdirSync(path, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) fixtureFiles(child);
    else if (entry.name.endsWith(".lil") || child === config) pinnedPaths.push(child);
  }
}
function pinnedInputs() {
  const files = pinnedPaths.map(identity).sort((a, b) => a.path.localeCompare(b.path));
  return { sha256: hash(JSON.stringify(files)), files };
}
function acceptedRun(runDirectory, side) {
  const receiptPath = join(runDirectory, "receipt.json");
  const receipt = readJson(receiptPath);
  assert.equal(receipt.passed, true, `${side} qualification did not pass`);
  assert.equal(receipt.inputsStable, true, `${side} qualification inputs were not stable`);
  const manifests = ["inputs-before.json", "inputs-after.json"].map(name => {
    const path = join(runDirectory, name);
    const manifest = readJson(path);
    assert.equal(hash(JSON.stringify(manifest.files)), manifest.sha256, `${side} ${name} digest`);
    assert.equal(manifest.sha256, receipt.inputSha256, `${side} ${name} qualification binding`);
    return { ...identity(path), manifest };
  });
  assert.deepEqual(manifests[0].manifest.files, manifests[1].manifest.files, `${side} qualified input manifests differ`);
  const binaries = receipt.binaries.filter(binary => basename(binary.path) === "lilscript");
  assert.equal(binaries.length, 1, `${side} receipt must identify exactly one compiler`);
  const recorded = binaries[0];
  assert(recorded.path.includes("/debug/"), `${side} compiler is not the qualified debug build`);
  const binary = side === "before"
    ? "/tmp/lilscript-checker-admission-baseline-20260919/lilscript"
    : resolve(root, recorded.path);
  const actual = identity(binary);
  assert.equal(actual.sha256, recorded.sha256, `${side} compiler hash differs from accepted receipt`);
  assert.equal(actual.bytes, recorded.bytes, `${side} compiler length differs from accepted receipt`);
  copyFileSync(receiptPath, join(outputDirectory, `${side}-qualification.json`));
  for (const name of ["inputs-before.json", "inputs-after.json"]) {
    copyFileSync(join(runDirectory, name), join(outputDirectory, `${side}-${name}`));
  }
  return {
    binary, sourceFiles: manifests[0].manifest.files,
    evidence: {
      qualification: identity(receiptPath), compilerInputsSha256: receipt.inputSha256,
      compiler: actual, qualifiedCompiler: recorded,
      inputManifests: manifests.map(({ manifest: _manifest, ...file }) => file),
    },
  };
}
function number(value, label) {
  assert(Number.isSafeInteger(value) && value >= 0, `${label} must be a nonnegative safe integer`);
  return value;
}
function measures(report, side, wallNs) {
  const phases = report.phases_ns;
  const old = side === "before";
  if (old) assert.equal(phases.discovery_parse_ns, undefined, "before report unexpectedly uses the new combined phase");
  else {
    assert.equal(phases.discovery_ns, undefined, "after report still has a separate discovery phase");
    assert.equal(phases.parse_ns, undefined, "after report still has a separate main parse phase");
  }
  return {
    discovery_parse_ns: old
      ? number(number(phases.discovery_ns, "discovery_ns") + number(phases.parse_ns, "parse_ns"), "discovery_ns + parse_ns")
      : number(phases.discovery_parse_ns, "discovery_parse_ns"),
    check_ns: number(phases.check_ns, "check_ns"),
    convert_ns: number(phases.convert_ns, "convert_ns"),
    frontend_release_ns: number(phases.frontend_release_ns, "frontend_release_ns"),
    adopt_ns: number(phases.adopt_ns, "adopt_ns"),
    source_release_ns: number(phases.source_release_ns, "source_release_ns"),
    total_ns: number(report.total_ns, "total_ns"),
    wall_ns: number(wallNs, "wall_ns"),
    frontend_logical_work: number(report.resources.frontend_logical_work, "frontend_logical_work"),
    source_buffer_capacity: number(report.resources.source_buffer_capacity, "source_buffer_capacity"),
    whole_compilation_peak_retained_bytes: number(report.resources.peak_retained_bytes, "peak_retained_bytes"),
  };
}
function verifyModules(report, entry) {
  const inputs = report.inputs;
  assert(Array.isArray(inputs.modules) && inputs.modules.length > 0, "missing input module manifest");
  assert.equal(inputs.modules[inputs.root]?.path, entry, "input manifest root is not the requested entry");
  for (const module of inputs.modules) {
    const pinned = receipt.pinnedInputs.files.find(file => resolve(root, file.path) === module.path);
    assert(pinned, `module was not pinned before execution: ${module.path}`);
    const actual = identity(module.path);
    assert.equal(actual.sha256, pinned.sha256, `module changed during execution: ${module.path}`);
    assert.equal(actual.sha256, module.sha256, `reported module digest differs from disk: ${module.path}`);
    assert.equal(actual.bytes, module.bytes, `reported module length differs from disk: ${module.path}`);
  }
  assert.equal(hash(JSON.stringify(inputs)), report.source_sha256, "reported input manifest digest mismatch");
  return inputs;
}
const receipt = {
  schema: 1,
  scope: "paired debug parse-once frontend diagnostic; exact delivery and source-manifest comparison",
  started: new Date().toISOString(), repetitions,
  schedule: "For each fixture, before/after on even repetitions, after/before on odd repetitions; no warmup exclusion",
  commandPolicy: { timeoutMs: 60_000, killAfterMs: 1000, maxBuffer: 16 * 1024 * 1024 },
  node: { version: process.version, executable: process.execPath },
  host: { platform: process.platform, arch: process.arch, logical_cpus: cpus().length, load_average_start: loadavg() },
  limitations: [
    "Debug builds and five local repetitions are not release-speed or fleet performance qualification.",
    "The shared host is not isolated and background CPU/memory load is not controlled; recorded load averages do not correct timings.",
    "No speed or compression improvement is asserted; output equality is required.",
    "Wall time includes supervised process startup and completion, not only compiler phases.",
    "Input and executable hashing is outside measured wall time and can warm filesystem caches; no cache flushing or cold-start claim.",
    "Before discovery_ns plus parse_ns is compared with after discovery_parse_ns; separate new discovery/parse times are not inferred.",
    "Logical work is an accounting tariff, not elapsed time; source capacities use each implementation's reported backing allocation.",
    "Peak retained bytes cover the whole compilation, not a frontend-only peak or process RSS.",
    "Remaining resource-accounting exclusions are preserved in every original compiler report.",
    "Both compilers are otherwise different qualified checkpoints; this is not an isolated compiler patch benchmark.",
    "Native-entry is compiled as JavaScript; this does not test native compilation or runtime equivalence.",
  ],
  trials: [], pairs: [], results: [], passed: false,
};
let initial;
try {
  fixtureFiles(fixtureDirectory);
  initial = pinnedInputs();
  receipt.pinnedInputs = initial;
  save("inputs-before.json", initial);
  receipt.node.required_version = `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`;
  assert.equal(process.version, receipt.node.required_version, "use the repository-pinned Node version");
  copyFileSync(fileURLToPath(import.meta.url), join(outputDirectory, "compare-parse-once.mjs"));
  copyFileSync(join(root, "finer/tools/bounded-command.mjs"), join(outputDirectory, "bounded-command.mjs"));
  const compilers = {
    before: acceptedRun(beforeDirectory, "before"),
    after: acceptedRun(afterDirectory, "after"),
  };
  receipt.compilers = Object.fromEntries(Object.entries(compilers).map(([side, compiler]) => [side, compiler.evidence]));
  receipt.config = identity(config);
  for (const pinned of initial.files.filter(file => file.path === ".nvmrc" || file.path.startsWith(relative(root, fixtureDirectory) + "/"))) {
    for (const [side, compiler] of Object.entries(compilers)) {
      const qualified = compiler.sourceFiles.find(file => file.path === pinned.path);
      assert(qualified, `${side} qualification did not pin fixture input ${pinned.path}`);
      assert.equal(qualified.sha256, pinned.sha256, `${side} qualified fixture differs from current input: ${pinned.path}`);
      assert.equal(qualified.bytes, pinned.bytes, `${side} qualified fixture length differs: ${pinned.path}`);
    }
  }
  const env = { ...process.env, RUST_BACKTRACE: "0" };
  receipt.environment = { cwd: root, overrides: { RUST_BACKTRACE: "0" }, inherited: true };
  for (const fixture of workloads) {
    let firstInputs, firstOutput;
    const metrics = { before: [], after: [] };
    for (let repetition = 0; repetition < repetitions; repetition++) {
      const sides = repetition % 2 === 0 ? ["before", "after"] : ["after", "before"];
      const paired = {};
      for (const side of sides) {
        const compiler = compilers[side];
        const label = `${fixture.name}-${repetition + 1}-${side}`;
        const commandArgs = [fixture.path, "--config", config, "--backend", "semantic", "--target", "js-module", "--mode", "development", "--explain", "json"];
        assert.equal(identity(compiler.binary).sha256, compiler.evidence.compiler.sha256, `${side} binary changed before ${label}`);
        const started = process.hrtime.bigint();
        const result = await runBoundedCommand(compiler.binary, commandArgs, { cwd: root, env, ...receipt.commandPolicy });
        const wallNs = Number(process.hrtime.bigint() - started);
        writeFileSync(join(outputDirectory, `${label}.stdout`), result.stdout);
        writeFileSync(join(outputDirectory, `${label}.stderr`), result.stderr);
        const trial = {
          fixture: fixture.name, repetition: repetition + 1, side, order: sides.indexOf(side),
          command: compiler.binary, args: commandArgs, status: result.status, signal: result.signal,
          error: result.error ? { message: result.error.message, code: result.error.code } : null,
          supervision: result.supervision, wall_ns: wallNs,
          stdout: identity(join(outputDirectory, `${label}.stdout`)),
          stderr: identity(join(outputDirectory, `${label}.stderr`)),
        };
        receipt.trials.push(trial);
        save("receipt.json", receipt);
        assert.equal(identity(compiler.binary).sha256, compiler.evidence.compiler.sha256, `${side} binary changed during ${label}`);
        assert.equal(result.error, undefined, `${label}: ${result.error?.message}`);
        assert.equal(result.status, 0, `${label}: unsupported input or compiler failure; see saved stderr`);
        assert.equal(result.signal, null, `${label}: process was signalled`);
        const report = JSON.parse(result.stderr.toString("utf8"));
        assert.equal(report.backend, "semantic");
        assert.equal(report.search.proposals, 0, `${label}: development mode unexpectedly searched`);
        const winner = report.artifacts[report.winners[2]];
        assert(winner, `${label}: no declared Brotli-objective delivery`);
        assert.equal(hash(result.stdout), winner.sha256, `${label}: delivered hash differs from report`);
        assert.equal(result.stdout.length, winner.raw, `${label}: delivered length differs from report`);
        const inputs = verifyModules(report, fixture.path);
        if (firstInputs) assert.deepEqual(inputs, firstInputs, `${label}: original input module manifest differs`);
        else firstInputs = inputs;
        if (firstOutput) assert(result.stdout.equals(firstOutput), `${label}: delivered bytes differ from first trial`);
        else firstOutput = result.stdout;
        trial.metrics = measures(report, side, wallNs);
        trial.source_buffer_accounting = report.resources.source_buffer_accounting;
        trial.frontend_phase_accounting = report.resources.frontend_phase_accounting;
        trial.input_manifest_sha256 = report.source_sha256;
        metrics[side].push(trial.metrics);
        paired[side] = { output: result.stdout, inputs, trial };
        save("receipt.json", receipt);
      }
      assert(paired.before.output.equals(paired.after.output), `${fixture.name} pair ${repetition + 1}: stdout bytes differ`);
      assert.deepEqual(paired.before.inputs, paired.after.inputs, `${fixture.name} pair ${repetition + 1}: input manifests differ`);
      receipt.pairs.push({
        fixture: fixture.name, repetition: repetition + 1, order: sides,
        delivered_sha256: hash(paired.before.output), raw: paired.before.output.length,
        exact_stdout_equal: true, exact_input_manifest_equal: true,
      });
    }
    const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
    const medians = {};
    for (const key of Object.keys(metrics.before[0])) {
      const before = median(metrics.before.map(row => row[key]));
      const after = median(metrics.after.map(row => row[key]));
      medians[key] = { before, after, difference: after - before };
    }
    receipt.results.push({ fixture: fixture.name, entry: identity(fixture.path), inputs: firstInputs, medians });
    save("receipt.json", receipt);
  }
  assert.equal(receipt.trials.length, workloads.length * repetitions * 2);
  assert.equal(receipt.pairs.length, workloads.length * repetitions);
  receipt.passed = true;
} catch (error) {
  receipt.failure = { message: error.message, stack: error.stack };
  process.exitCode = 1;
  console.error(error.message);
} finally {
  if (initial) {
    try {
      const final = pinnedInputs();
      save("inputs-after.json", final);
      receipt.inputsStable = initial.sha256 === final.sha256;
      if (!receipt.inputsStable) {
        receipt.passed = false;
        receipt.failure ??= { message: "Pinned script, runner, Node or fixture inputs changed during comparison" };
        process.exitCode = 1;
      }
    } catch (error) {
      receipt.inputsStable = false;
      receipt.passed = false;
      receipt.failure ??= { message: error.message, stack: error.stack };
      process.exitCode = 1;
    }
  }
  receipt.completed = new Date().toISOString();
  receipt.host.load_average_end = loadavg();
  save("receipt.json", receipt);
  console.log(JSON.stringify({ directory: outputDirectory, passed: receipt.passed, inputsStable: receipt.inputsStable ?? false }));
}
