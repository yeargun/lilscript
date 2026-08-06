import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { arch, platform, release } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { build } from "esbuild";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const buildRoot = join(labRoot, "build");
const compiler = join(repoRoot, "target/release/lilscript");
const cargo = process.env.CARGO ?? "cargo";
const closure = join(
  labRoot,
  "node_modules/.bin",
  platform() === "win32" ? "google-closure-compiler.cmd" : "google-closure-compiler",
);
const verifyOnly = process.argv.includes("--verify-only");
const warmups = verifyOnly ? 0 : Number(process.env.BENCH_WARMUPS ?? 2);
const samples = verifyOnly ? 1 : Number(process.env.BENCH_SAMPLES ?? 9);

const cases = [
  {
    name: "reactive-store",
    jsEntry: "cases/reactive-store/js/main.js",
    closureEntry: "cases/reactive-store/closure/main.js",
    lilEntry: "cases/reactive-store/lil/main.lil",
    hand: "cases/reactive-store/hand.js",
    expected: "cases/reactive-store/expected.txt",
  },
  {
    name: "event-pipeline",
    jsEntry: "cases/event-pipeline/js/main.js",
    closureEntry: "cases/event-pipeline/closure/main.js",
    lilEntry: "cases/event-pipeline/lil/main.lil",
    hand: "cases/event-pipeline/hand.js",
    expected: "cases/event-pipeline/expected.txt",
  },
  {
    name: "binary-telemetry",
    jsEntry: "cases/binary-telemetry/js/main.js",
    closureEntry: "cases/binary-telemetry/closure/main.js",
    lilEntry: "cases/binary-telemetry/lil/main.lil",
    hand: "cases/binary-telemetry/hand.js",
    expected: "cases/binary-telemetry/expected.txt",
  },
  {
    name: "module-pricing",
    jsEntry: "cases/module-pricing/js/main.js",
    closureEntry: "cases/module-pricing/closure/main.js",
    lilEntry: "cases/module-pricing/lil/main.lil",
    hand: "cases/module-pricing/hand.js",
    expected: "cases/module-pricing/expected.txt",
  },
];

function command(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? repoRoot,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    timeout: options.timeout ?? 180_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")} failed (${result.status})\n${result.stdout}${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

function commandExists(program) {
  const result = spawnSync(program, ["--version"], { encoding: "utf8" });
  return !result.error && result.status === 0;
}

function normalize(code) {
  return code.replaceAll("\r\n", "\n").trimEnd();
}

async function writeNormalized(path, code) {
  await writeFile(path, normalize(code));
}

async function bundle(entry, minify) {
  const result = await build({
    absWorkingDir: labRoot,
    bundle: true,
    entryPoints: [entry],
    format: "iife",
    legalComments: "none",
    minify,
    platform: "browser",
    target: "es2021",
    treeShaking: true,
    write: false,
  });
  return result.outputFiles[0].text;
}

function execute(path) {
  const result = spawnSync(process.execPath, [path], {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 1024 * 1024,
    timeout: 180_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${relative(labRoot, path)} failed (${result.status})\n${result.stderr}`);
  }
  if (result.stderr) {
    throw new Error(`${relative(labRoot, path)} wrote to stderr:\n${result.stderr}`);
  }
  return normalize(result.stdout);
}

function executeNative(path) {
  const result = spawnSync(path, [], {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 1024 * 1024,
    timeout: 180_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${relative(labRoot, path)} failed (${result.status})\n${result.stderr}`);
  }
  if (result.stderr) {
    throw new Error(`${relative(labRoot, path)} wrote to stderr:\n${result.stderr}`);
  }
  return normalize(result.stdout);
}

function timedExecution(path) {
  const start = process.hrtime.bigint();
  execute(path);
  return Number(process.hrtime.bigint() - start) / 1_000_000;
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function metrics(code) {
  const bytes = Buffer.from(normalize(code));
  return {
    raw: bytes.length,
    gzip: gzipSync(bytes, { level: 9, mtime: 0 }).length,
    brotli: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
  };
}

async function sourceFiles(directory, extension) {
  const found = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) found.push(...(await sourceFiles(path, extension)));
    else if (entry.name.endsWith(extension)) found.push(path);
  }
  return found.sort();
}

async function sourceBytes(entry, extension) {
  const files = await sourceFiles(dirname(entry), extension);
  const contents = await Promise.all(files.map((path) => readFile(path)));
  return contents.reduce((total, contents) => total + contents.length, 0);
}

function packageVersion(name) {
  return JSON.parse(readFileSync(join(labRoot, `node_modules/${name}/package.json`), "utf8"))
    .version;
}

function percent(value, baseline) {
  return `${value <= baseline ? "" : "+"}${(((value / baseline) - 1) * 100).toFixed(1)}%`;
}

function renderReport(results, metadata) {
  const lines = [
    "# Application benchmark results",
    "",
    `Generated on ${metadata.generatedAt} with LilScript \`${metadata.compilerRevision}\`, Node ` +
      `\`${metadata.node}\`, esbuild \`${metadata.esbuild}\`, and Google Closure Compiler ` +
      `\`${metadata.closure}\` on \`${metadata.system}\`.`,
    "",
    "Every JavaScript artifact and LilScript native executable passed the same checked-in stdout contract. Negative deltas are smaller or faster than Closure ADVANCED.",
    "",
    `Ecosystem JavaScript lanes use Alien Signals \`${metadata.alienSignals}\` and mitt \`${metadata.mitt}\`.`,
    "",
    "## Source size",
    "",
    "Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.",
    "",
    "| Workload | JS app source | Closure-friendly source | LilScript app source | Hand-specialized JS |",
    "| --- | ---: | ---: | ---: | ---: |",
  ];

  for (const result of results) {
    lines.push(
      `| ${result.name} | ${result.source.js} | ${result.source.closure} | ${result.source.lilscript} | ${result.source.hand} |`,
    );
  }

  for (const result of results) {
    const closureRow = result.artifacts.find((artifact) => artifact.id === "closure");
    lines.push(
      "",
      `## ${result.name}`,
      "",
      `Expected output: \`${result.expected.replaceAll("\n", " ")}\``,
      "",
      "| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |",
      "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    );
    for (const artifact of result.artifacts) {
      lines.push(
        `| ${artifact.label} | ${artifact.raw} | ${artifact.gzip} | ${artifact.brotli} | ` +
          `${percent(artifact.brotli, closureRow.brotli)} | ${artifact.medianMs.toFixed(2)} | ` +
          `${percent(artifact.medianMs, closureRow.medianMs)} |`,
      );
    }
  }

  const ids = results[0].artifacts.map((artifact) => artifact.id);
  lines.push(
    "",
    "## Corpus totals",
    "",
    "Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.",
    "",
    "| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |",
    "| --- | ---: | ---: | ---: | ---: | ---: |",
  );
  const closureBrotli = results.reduce(
    (total, result) => total + result.artifacts.find((artifact) => artifact.id === "closure").brotli,
    0,
  );
  for (const id of ids) {
    const rows = results.map((result) => result.artifacts.find((artifact) => artifact.id === id));
    const raw = rows.reduce((total, row) => total + row.raw, 0);
    const gzip = rows.reduce((total, row) => total + row.gzip, 0);
    const brotli = rows.reduce((total, row) => total + row.brotli, 0);
    const ratios = results.map((result, index) => {
      const closureTime = result.artifacts.find((artifact) => artifact.id === "closure").medianMs;
      return rows[index].medianMs / closureTime;
    });
    const runtimeRatio = Math.exp(
      ratios.reduce((total, ratio) => total + Math.log(ratio), 0) / ratios.length,
    );
    lines.push(
      `| ${rows[0].label} | ${raw} | ${gzip} | ${brotli} | ${percent(brotli, closureBrotli)} | ` +
        `${runtimeRatio.toFixed(3)}x |`,
    );
  }

  lines.push(
    "",
    "## Interpretation limits",
    "",
    "- `reactive-store` and `event-pipeline` compare complete app behavior, not complete library APIs.",
    "- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.",
    "- Closure receives a readable app-specific implementation, bundled without minification before `ADVANCED` compilation.",
    "- Fresh-process runtime includes Node startup and is intended to catch large regressions, not establish engine-level causality.",
    "- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.",
    "",
  );
  return lines.join("\n");
}

await rm(buildRoot, { force: true, recursive: true });
await mkdir(buildRoot, { recursive: true });
if (commandExists(cargo)) {
  command(cargo, ["build", "--release", "--bin", "lilscript"]);
} else if (!existsSync(compiler)) {
  throw new Error("Cargo is unavailable and target/release/lilscript does not exist");
} else {
  console.warn("Cargo is unavailable; using the existing target/release/lilscript binary");
}

const results = [];
for (const benchmark of cases) {
  const directory = join(buildRoot, benchmark.name);
  await mkdir(directory, { recursive: true });
  const paths = {
    raw: join(directory, "js-raw.js"),
    esbuild: join(directory, "js-esbuild.js"),
    closure: join(directory, "js-closure.js"),
    hand: join(directory, "js-hand.js"),
    lilscript: join(directory, "lilscript.js"),
  };
  const lilscriptBase = join(directory, "lilscript");

  const rawCode = await bundle(benchmark.jsEntry, false);
  await writeNormalized(paths.raw, rawCode);
  await writeNormalized(paths.esbuild, await bundle(benchmark.jsEntry, true));
  await writeNormalized(paths.hand, await readFile(join(labRoot, benchmark.hand), "utf8"));
  command(compiler, [
    join(labRoot, benchmark.lilEntry),
    "--target",
    "all",
    "-o",
    lilscriptBase,
  ]);
  const closureInput = join(directory, "closure-input.js");
  await writeNormalized(closureInput, await bundle(benchmark.closureEntry, false));
  command(closure, [
    "--js",
    closureInput,
    "--js_output_file",
    paths.closure,
    "--compilation_level",
    "ADVANCED",
    "--language_in",
    "ECMASCRIPT_2021",
    "--language_out",
    "ECMASCRIPT_2021",
    "--warning_level",
    "QUIET",
    "--emit_use_strict=false",
    "--rewrite_polyfills=false",
  ]);

  const expected = normalize(await readFile(join(labRoot, benchmark.expected), "utf8"));
  const nativeOutput = executeNative(lilscriptBase);
  if (nativeOutput !== expected) {
    throw new Error(
      `${benchmark.name}/native output mismatch\nexpected: ${JSON.stringify(expected)}\nactual:   ${JSON.stringify(nativeOutput)}`,
    );
  }
  const artifacts = [
    { id: "raw", label: "JS raw bundle", path: paths.raw },
    { id: "esbuild", label: "JS esbuild", path: paths.esbuild },
    { id: "closure", label: "JS Closure ADVANCED", path: paths.closure },
    { id: "hand", label: "JS hand-specialized", path: paths.hand },
    { id: "lilscript", label: "LilScript", path: paths.lilscript },
  ];

  for (const artifact of artifacts) {
    const actual = execute(artifact.path);
    if (actual !== expected) {
      throw new Error(
        `${benchmark.name}/${artifact.id} output mismatch\nexpected: ${JSON.stringify(expected)}\nactual:   ${JSON.stringify(actual)}`,
      );
    }
    for (let index = 0; index < warmups; index += 1) timedExecution(artifact.path);
    const timings = [];
    for (let index = 0; index < samples; index += 1) timings.push(timedExecution(artifact.path));
    Object.assign(artifact, metrics(await readFile(artifact.path, "utf8")), {
      medianMs: median(timings),
      samplesMs: timings,
    });
    delete artifact.path;
  }

  results.push({
    name: benchmark.name,
    expected,
    source: {
      js: await sourceBytes(join(labRoot, benchmark.jsEntry), ".js"),
      closure: await sourceBytes(join(labRoot, benchmark.closureEntry), ".js"),
      lilscript: await sourceBytes(join(labRoot, benchmark.lilEntry), ".lil"),
      hand: Buffer.byteLength(await readFile(join(labRoot, benchmark.hand), "utf8")),
    },
    artifacts,
  });
  console.log(
    `${benchmark.name}: behavior verified across ${artifacts.length} JavaScript artifacts and native`,
  );
}

const packageJson = JSON.parse(await readFile(join(labRoot, "package.json"), "utf8"));
const metadata = {
  generatedAt: new Date().toISOString(),
  compilerRevision: command("git", ["rev-parse", "--short", "HEAD"]),
  node: process.version,
  esbuild: packageJson.devDependencies.esbuild,
  closure: packageVersion("google-closure-compiler"),
  alienSignals: packageVersion("alien-signals"),
  mitt: packageVersion("mitt"),
  system: `${platform()} ${release()} ${arch()}`,
  warmups,
  samples,
};
await writeFile(join(buildRoot, "results.json"), `${JSON.stringify({ metadata, results }, null, 2)}\n`);
if (!verifyOnly) {
  await writeFile(join(labRoot, "RESULTS.md"), renderReport(results, metadata));
}
console.log(`verified ${results.length} workloads; ${samples} measured execution(s) per artifact`);
