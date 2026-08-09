import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { arch, platform, release } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { build } from "esbuild";
import { build as viteBuild } from "vite";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const buildRoot = join(labRoot, "build");
const webResults = join(repoRoot, "web/src/benchmark-results.json");
const compiler = join(repoRoot, "target/release/lilscript");
const timingRunner = join(labRoot, "timing-runner.mjs");
const cargo = process.env.CARGO ?? "cargo";
const closure = join(
  labRoot,
  "node_modules/.bin",
  platform() === "win32" ? "google-closure-compiler.cmd" : "google-closure-compiler",
);
const verifyOnly = process.argv.includes("--verify-only");
const warmups = verifyOnly ? 0 : Number(process.env.BENCH_WARMUPS ?? 5);
const samples = verifyOnly ? 1 : Number(process.env.BENCH_SAMPLES ?? 25);
const comparableArtifactIds = ["reference", "esbuild", "closure", "hand", "lilscript"];

const cases = [
  {
    name: "reactive-store",
    title: "Reactive store",
    referenceEntry: "cases/reactive-store/closure/main.js",
    ecosystemRoot: "cases/reactive-store/js",
    ecosystemLabel: "Alien Signals via Vite",
    lilEntry: "cases/reactive-store/lil/main.lil",
    hand: "cases/reactive-store/hand.js",
    expected: "cases/reactive-store/expected.txt",
  },
  {
    name: "event-pipeline",
    title: "Event pipeline",
    referenceEntry: "cases/event-pipeline/closure/main.js",
    ecosystemRoot: "cases/event-pipeline/js",
    ecosystemLabel: "mitt via Vite",
    lilEntry: "cases/event-pipeline/lil/main.lil",
    hand: "cases/event-pipeline/hand.js",
    expected: "cases/event-pipeline/expected.txt",
  },
  {
    name: "binary-telemetry",
    title: "Binary telemetry",
    referenceEntry: "cases/binary-telemetry/js/main.js",
    lilEntry: "cases/binary-telemetry/lil/main.lil",
    hand: "cases/binary-telemetry/hand.js",
    expected: "cases/binary-telemetry/expected.txt",
  },
  {
    name: "module-pricing",
    title: "Module pricing",
    referenceEntry: "cases/module-pricing/js/main.js",
    lilEntry: "cases/module-pricing/lil/main.lil",
    hand: "cases/module-pricing/hand.js",
    expected: "cases/module-pricing/expected.txt",
  },
  {
    name: "motion-values",
    title: "Animation value kernel",
    referenceEntry: "cases/motion-values/closure/main.js",
    ecosystemRoot: "cases/motion-values/js",
    ecosystemLabel: "Motion value and spring APIs via Vite",
    ecosystemExpected: "cases/motion-values/ecosystem-expected.txt",
    lilEntry: "cases/motion-values/lil/main.lil",
    specializedLilEntry: "cases/motion-values/lil-specialized/main.lil",
    hand: "cases/motion-values/hand.js",
    expected: "cases/motion-values/expected.txt",
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

async function viteProductionBundle(root, outDir) {
  await viteBuild({
    root,
    base: "./",
    configFile: false,
    logLevel: "silent",
    build: {
      outDir,
      emptyOutDir: true,
      manifest: true,
      minify: true,
      modulePreload: { polyfill: false },
      target: "baseline-widely-available",
    },
  });

  const manifest = JSON.parse(await readFile(join(outDir, ".vite/manifest.json"), "utf8"));
  const entry = Object.values(manifest).find((item) => item.isEntry);
  if (!entry) throw new Error(`Vite emitted no entry for ${relative(labRoot, root)}`);

  const files = (await sourceFiles(outDir)).filter((path) => /\.(?:css|html|js)$/.test(path));
  const totals = { raw: 0, gzip: 0, brotli: 0 };
  for (const path of files) {
    const fileMetrics = metrics(await readFile(path, "utf8"));
    totals.raw += fileMetrics.raw;
    totals.gzip += fileMetrics.gzip;
    totals.brotli += fileMetrics.brotli;
  }

  return {
    entry: join(outDir, entry.file),
    files: files.map((path) => relative(outDir, path)),
    ...totals,
  };
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

function measureExecutions(paths) {
  return JSON.parse(
    command(process.execPath, [timingRunner, String(warmups), String(samples), ...paths], {
      cwd: labRoot,
    }),
  );
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
    else if (extension === undefined || entry.name.endsWith(extension)) found.push(path);
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
      `\`${metadata.node}\`, Vite \`${metadata.vite}\`, esbuild \`${metadata.esbuild}\`, and ` +
      `Google Closure Compiler \`${metadata.closure}\` on \`${metadata.system}\`.`,
    "",
    "This report contains two deliberately separate datasets. Compiler rows use a readable JavaScript reference and a LilScript implementation with the same app algorithm and abstraction scope. Ecosystem rows build real npm packages with Vite and are never included in compiler totals.",
    "",
    "Every emitted artifact passed its checked-in stdout contract. That rejects observed behavior mismatches for these inputs; it does not prove complete semantic or library API equivalence.",
    "",
    `Context-only ecosystem builds use Alien Signals \`${metadata.alienSignals}\`, mitt \`${metadata.mitt}\`, and Motion \`${metadata.motion}\`.`,
    "",
    "## Source size",
    "",
    "Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.",
    "",
    "| Workload | Reference JS | LilScript | Hand-specialized JS |",
    "| --- | ---: | ---: | ---: |",
  ];

  for (const result of results) {
    lines.push(
      `| ${result.title} | ${result.source.reference} | ${result.source.lilscript} | ${result.source.hand} |`,
    );
  }

  for (const result of results) {
    const closureRow = result.artifacts.find((artifact) => artifact.id === "closure");
    lines.push(
      "",
      `## ${result.title}`,
      "",
      `Expected output: \`${result.expected.replaceAll("\n", " ")}\``,
      "",
      "Comparable compiler artifacts:",
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
    if (result.ecosystem) {
      lines.push(
        "",
        `Context-only production build: **${result.ecosystem.label}**. This uses a different library implementation and is excluded from every compiler delta and total.`,
        "",
        `Vite output contract: \`${result.ecosystem.expected.replaceAll("\n", " ")}\``,
        "",
        "| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |",
        "| --- | ---: | ---: | ---: | ---: |",
        `| ${result.ecosystem.files.join("<br>")} | ${result.ecosystem.raw} | ${result.ecosystem.gzip} | ${result.ecosystem.brotli} | ${result.ecosystem.medianMs.toFixed(2)} |`,
      );
    }
  }

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
  for (const id of comparableArtifactIds) {
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
    "- Hand-specialized JavaScript is an oracle for expert whole-program rewriting, not source given to Closure or LilScript.",
    "- Real package builds are Vite context measurements only. No package row is compared with a specialized rewrite or included in corpus totals.",
    "- The Motion compiler workload matches the selected numeric mix/wrap/stagger and underdamped-spring equations and digest; LilScript still does not implement Motion's package API. A complete claim requires the public package surface and upstream behavioral tests, including DOM, timing, cancellation, gestures, scrolling, SVG, and React entry points.",
    "- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.",
    "- Closure receives the exact readable JavaScript reference used by the unminified and esbuild rows.",
    "- Matching one deterministic stdout contract can have false negatives; it is regression evidence, not a proof of general equivalence.",
    "- The checked methodology gate requires every LilScript workload to be no larger than Closure in raw, gzip-9, and Brotli-11 bytes; full 20+-sample runs also require median runtime within 5% of Closure.",
    "- Runtime is repeated cache-busted module parsing plus execution inside one dedicated Node process per artifact. It excludes process startup but is not a browser-frame benchmark.",
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
    reference: join(directory, "js-reference.js"),
    esbuild: join(directory, "js-esbuild.js"),
    closure: join(directory, "js-closure.js"),
    hand: join(directory, "js-hand.js"),
    lilscript: join(directory, "lilscript.js"),
  };
  const lilscriptBase = join(directory, "lilscript");

  const referenceCode = await bundle(benchmark.referenceEntry, false);
  await writeNormalized(paths.reference, referenceCode);
  await writeNormalized(paths.esbuild, await bundle(benchmark.referenceEntry, true));
  await writeNormalized(paths.hand, await readFile(join(labRoot, benchmark.hand), "utf8"));
  command(compiler, [
    join(labRoot, benchmark.lilEntry),
    "--target",
    "all",
    "-o",
    lilscriptBase,
  ]);
  const closureInput = join(directory, "closure-input.js");
  await writeNormalized(closureInput, referenceCode);
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
    { id: "reference", label: "Reference JS bundle", path: paths.reference },
    { id: "esbuild", label: "Reference JS esbuild", path: paths.esbuild },
    { id: "closure", label: "JS Closure ADVANCED", path: paths.closure },
    { id: "hand", label: "JS hand-specialized", path: paths.hand },
    { id: "lilscript", label: "LilScript", path: paths.lilscript },
  ];

  let specializedNative;
  if (benchmark.specializedLilEntry) {
    const specializedBase = join(directory, "lilscript-specialized");
    command(compiler, [
      join(labRoot, benchmark.specializedLilEntry),
      "--target",
      "all",
      "-o",
      specializedBase,
    ]);
    specializedNative = specializedBase;
    artifacts.push({
      id: "lilscript-specialized",
      label: "LilScript specialized source (diagnostic)",
      path: `${specializedBase}.js`,
    });
  }

  for (const artifact of artifacts) {
    const actual = execute(artifact.path);
    if (actual !== expected) {
      throw new Error(
        `${benchmark.name}/${artifact.id} output mismatch\nexpected: ${JSON.stringify(expected)}\nactual:   ${JSON.stringify(actual)}`,
      );
    }
    Object.assign(artifact, metrics(await readFile(artifact.path, "utf8")));
  }

  if (artifacts.slice(0, comparableArtifactIds.length).some(
    (artifact, index) => artifact.id !== comparableArtifactIds[index]
  )) {
    throw new Error(`${benchmark.name}: comparable artifact scope changed`);
  }

  if (specializedNative && executeNative(specializedNative) !== expected) {
    throw new Error(`${benchmark.name}/specialized native output mismatch`);
  }

  let ecosystem;
  if (benchmark.ecosystemRoot) {
    const production = await viteProductionBundle(
      join(labRoot, benchmark.ecosystemRoot),
      join(directory, "vite-production"),
    );
    const ecosystemExpected = benchmark.ecosystemExpected
      ? normalize(await readFile(join(labRoot, benchmark.ecosystemExpected), "utf8"))
      : expected;
    const actual = execute(production.entry);
    if (actual !== ecosystemExpected) {
      throw new Error(
        `${benchmark.name}/vite ecosystem output mismatch\nexpected: ${JSON.stringify(ecosystemExpected)}\nactual:   ${JSON.stringify(actual)}`,
      );
    }
    ecosystem = {
      label: benchmark.ecosystemLabel,
      expected: ecosystemExpected,
      files: production.files,
      raw: production.raw,
      gzip: production.gzip,
      brotli: production.brotli,
      path: production.entry,
    };
  }

  const timedPaths = artifacts.map((artifact) => artifact.path);
  if (ecosystem) timedPaths.push(ecosystem.path);
  const timingGroups = measureExecutions(timedPaths);
  for (const [index, artifact] of artifacts.entries()) {
    artifact.samplesMs = timingGroups[index];
    artifact.medianMs = median(artifact.samplesMs);
    delete artifact.path;
  }
  if (ecosystem) {
    ecosystem.samplesMs = timingGroups.at(-1);
    ecosystem.medianMs = median(ecosystem.samplesMs);
    delete ecosystem.path;
  }

  results.push({
    name: benchmark.name,
    title: benchmark.title,
    expected,
    source: {
      reference: await sourceBytes(join(labRoot, benchmark.referenceEntry), ".js"),
      lilscript: await sourceBytes(join(labRoot, benchmark.lilEntry), ".lil"),
      hand: Buffer.byteLength(await readFile(join(labRoot, benchmark.hand), "utf8")),
    },
    artifacts,
    ecosystem,
  });
  console.log(
    `${benchmark.name}: ${artifacts.length} comparable/diagnostic JavaScript artifacts, native, and ${ecosystem ? "Vite ecosystem" : "no ecosystem"} verified`,
  );
}

const packageJson = JSON.parse(await readFile(join(labRoot, "package.json"), "utf8"));
const metadata = {
  generatedAt: new Date().toISOString(),
  compilerRevision: command("git", ["rev-parse", "--short", "HEAD"]),
  node: process.version,
  vite: packageJson.devDependencies.vite,
  esbuild: packageJson.devDependencies.esbuild,
  closure: packageVersion("google-closure-compiler"),
  alienSignals: packageVersion("alien-signals"),
  mitt: packageVersion("mitt"),
  motion: packageVersion("motion"),
  system: `${platform()} ${release()} ${arch()}`,
  warmups,
  samples,
};
await writeFile(join(buildRoot, "results.json"), `${JSON.stringify({ metadata, results }, null, 2)}\n`);
if (!verifyOnly) {
  await writeFile(join(labRoot, "RESULTS.md"), renderReport(results, metadata));
  const publishedResults = results.map((result) => ({
    ...result,
    artifacts: result.artifacts.map(({ samplesMs, ...artifact }) => artifact),
    ecosystem: result.ecosystem
      ? (({ samplesMs, ...ecosystem }) => ecosystem)(result.ecosystem)
      : undefined,
  }));
  await writeFile(
    webResults,
    `${JSON.stringify({ metadata, results: publishedResults }, null, 2)}\n`,
  );
}
console.log(`verified ${results.length} workloads; ${samples} measured execution(s) per artifact`);
