import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { arch, cpus, platform, release } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const worker = join(root, "benchmark-exact-worker.mjs");
const libraries = ["nanoid", "mitt", "clsx", "gl-matrix"];
const implementations = ["npm", "lilscript"];
const modes = ["performance", "memory"];
const rounds = Number(process.env.BENCH_ROUNDS ?? 5);
const eligibleLibraries = ["nanoid", "mitt", "clsx", "gl-matrix"];
const maxEligibleTimeRatio = 1.05;
const maxEligibleMemoryRatio = 1.05;
const runtimeObjective = "brotli";
const runtimeConfig = "lilscript.toml";
const sizeReportPath = join(root, "build/results.json");
const sizeReportBytes = readFileSync(sizeReportPath);
const sizeRows = JSON.parse(sizeReportBytes);
const measuredRows = sizeRows.filter((row) => !row.external);
assert.ok(measuredRows.length > 0, "fresh popular size rows are required");
const codecs = measuredRows[0].codecs;
const compiler = measuredRows[0].compiler;
assert.equal(codecs?.implementation, "lilscript-codec");
assert.match(codecs?.scorer?.sha256 ?? "", /^[a-f0-9]{64}$/u);
assert.match(compiler?.sha256 ?? "", /^[a-f0-9]{64}$/u);
for (const row of measuredRows) {
  assert.deepEqual(row.codecs, codecs, `${row.id} codec provenance`);
  assert.deepEqual(row.compiler, compiler, `${row.id} compiler provenance`);
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
function artifactDigest(path) {
  const stat = statSync(path);
  if (stat.isFile()) return sha256(readFileSync(path));
  const hash = createHash("sha256");
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true }).sort(
      (left, right) => left.name.localeCompare(right.name),
    )) {
      const child = join(directory, entry.name);
      if (entry.isDirectory()) visit(child);
      else if (entry.isFile()) {
        hash.update(child.slice(path.length + 1));
        hash.update("\0");
        hash.update(readFileSync(child));
        hash.update("\0");
      }
    }
  };
  visit(path);
  return hash.digest("hex");
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)];
}

function sample(library, implementation, mode) {
  const result = spawnSync(
    process.execPath,
    ["--expose-gc", worker, library, implementation, mode],
    { cwd: root, encoding: "utf8" },
  );
  if (result.status !== 0) {
    throw new Error(
      result.stderr ||
        result.stdout ||
        `${library}/${implementation}/${mode} failed`,
    );
  }
  return JSON.parse(result.stdout);
}

const samples = {};
for (const library of libraries) {
  samples[library] = {};
  for (const mode of modes) {
    samples[library][mode] = { npm: [], lilscript: [] };
    for (let round = 0; round < rounds; round += 1) {
      const order =
        round % 2 === 0 ? implementations : [...implementations].reverse();
      for (const implementation of order) {
        samples[library][mode][implementation].push(
          sample(library, implementation, mode),
        );
      }
    }
    assert.deepEqual(
      samples[library][mode].lilscript.map(({ checksum }) => checksum),
      samples[library][mode].npm.map(({ checksum }) => checksum),
      `${library}/${mode} workloads differ`,
    );
  }
}

const results = Object.fromEntries(
  libraries.map((library) => {
    const npmMs = median(
      samples[library].performance.npm.map(({ milliseconds }) => milliseconds),
    );
    const lilMs = median(
      samples[library].performance.lilscript.map(
        ({ milliseconds }) => milliseconds,
      ),
    );
    const npmBytes = median(
      samples[library].memory.npm.map(({ bytes }) => bytes),
    );
    const lilBytes = median(
      samples[library].memory.lilscript.map(({ bytes }) => bytes),
    );
    return [
      library,
      {
        performance: { npmMs, lilscriptMs: lilMs, ratio: lilMs / npmMs },
        retainedMemory: {
          npmBytes,
          lilscriptBytes: lilBytes,
          ratio: lilBytes / npmBytes,
        },
      },
    ];
  }),
);

for (const library of eligibleLibraries) {
  assert.ok(
    results[library].performance.ratio <= maxEligibleTimeRatio,
    `${library} throughput regression: ${results[library].performance.ratio.toFixed(3)}`,
  );
  assert.ok(
    results[library].retainedMemory.ratio <= maxEligibleMemoryRatio,
    `${library} retained-memory regression: ${results[library].retainedMemory.ratio.toFixed(3)}`,
  );
}

const report = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  runtimeObjective,
  runtimeConfig,
  codecs,
  compiler,
  sizeEvidence: {
    path: "benchmarks/popular/build/results.json",
    sha256: sha256(sizeReportBytes),
  },
  workload: {
    path: "benchmarks/popular/benchmark-exact-worker.mjs",
    sha256: artifactDigest(worker),
  },
  candidateArtifacts: {
    nanoid: artifactDigest(join(root, "apps/nanoid/lil/api.js")),
    mitt: artifactDigest(join(root, "apps/mitt/lil/api.js")),
    clsx: artifactDigest(join(root, "apps/clsx/lil/api.js")),
    "gl-matrix": artifactDigest(join(root, "build/gl-matrix-pm")),
  },
  environment: {
    node: process.version,
    platform: `${platform()} ${release()} ${arch()}`,
    cpu: cpus()[0]?.model ?? "unknown",
  },
  rounds,
  method: {
    processIsolation: "one fresh Node --expose-gc process per sample",
    statistic: "median",
    retainedMemory:
      "heapUsed plus ArrayBuffer delta after forced GC with equivalent results retained",
    eligibilityGate: {
      libraries: eligibleLibraries,
      maxMedianTimeRatio: maxEligibleTimeRatio,
      maxMedianRetainedMemoryRatio: maxEligibleMemoryRatio,
      excluded: {},
    },
    targets: {
      nanoid: "nanoid/index.browser.js",
      mitt: "mitt root default export",
      clsx: "clsx root default export",
      "gl-matrix": "gl-matrix root entrypoint in Float32Array mode",
    },
  },
  results,
  samples,
};
writeFileSync(
  join(root, "build/performance-memory.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

const rows = libraries.map((library) => {
  const result = results[library];
  return `| ${library} | ${result.performance.npmMs.toFixed(3)} | ${result.performance.lilscriptMs.toFixed(3)} | ${result.performance.ratio.toFixed(3)} | ${result.retainedMemory.npmBytes} | ${result.retainedMemory.lilscriptBytes} | ${result.retainedMemory.ratio.toFixed(3)} |`;
});
const markdown = `# Selected-entrypoint performance and retained-memory checks

Median of ${rounds} isolated Node ${process.version} processes. LilScript runtime uses the Brotli-objective modules emitted by the preceding size build with explicit \`${runtimeConfig}\`; raw and gzip diagnostics do not select a runtime artifact. Time workloads use identical inputs and checksums; retained memory is the unclamped heap-used delta after forced GC while keeping equivalent results or emitter state alive. Nano ID compares the same published browser entrypoint used by the size lane, not its distinct pooled Node entrypoint. Ratios are LilScript / npm. Eligible exact ports must remain at or below ${maxEligibleTimeRatio.toFixed(2)} for both median time and retained memory.

| Project | npm ms | LilScript ms | Time ratio | npm retained B | LilScript retained B | Memory ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
${rows.join("\n")}
`;
writeFileSync(join(root, "PERFORMANCE.md"), markdown);
console.log(markdown);
