import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve } from "node:path";
import { arch, cpus, homedir, platform, release } from "node:os";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { build as esbuild } from "esbuild";
import { build as viteBuild } from "vite";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const buildRoot = join(labRoot, "build");
const compiler = join(repoRoot, "target/release/lilscript");
const cargo = process.env.CARGO ?? join(homedir(), ".cargo/bin/cargo");
const closure = join(labRoot, "node_modules/.bin/google-closure-compiler");
const webResults = join(repoRoot, "web/src/scenario-results.json");
const cases = [
  {
    id: "login-risk",
    title: "Login risk scoring",
    summary: "A login audit combines edit distance, two production hash algorithms, clamping, private result records, and deterministic batching.",
    packages: ["js-levenshtein", "string-hash", "@emotion/hash", "clamp"],
  },
  {
    id: "animation-timeline",
    title: "Animation timeline",
    summary: "A frame scheduler combines cubic-bezier easing, steps, interpolation, clamping, and private frame records.",
    packages: ["@motionone/easing", "clamp", "lerp"],
  },
  {
    id: "geometry-hit-test",
    title: "Geometry hit testing",
    summary: "A drawing-style hit-test batch combines adaptive robust predicates, confidence clamping, and private hit records.",
    packages: ["robust-predicates", "clamp"],
  },
  {
    id: "property-ledger",
    title: "Closed property ledger",
    summary: "A focused host-boundary contract keeps aggregate objects alive while declaring their field names unobservable, making private-property mangling measurable instead of optimized away.",
    packages: [],
    category: "mangling",
    host: "apps/property-ledger/host.mjs",
    native: false,
  },
];

function command(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? labRoot,
    encoding: "utf8",
    timeout: options.timeout ?? 300_000,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed (${result.status})\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

function normalize(value) {
  return value.replaceAll("\r\n", "\n").trimEnd();
}

function execute(path, host) {
  return normalize(command(process.execPath, host ? ["--import", host, path] : [path]));
}

function metrics(contents) {
  const bytes = Buffer.isBuffer(contents) ? contents : Buffer.from(contents);
  return {
    raw: bytes.length,
    gzip: gzipSync(bytes, { level: 9, mtime: 0 }).length,
    brotli: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
  };
}

async function onlyJavaScript(directory) {
  const names = (await readdir(directory)).filter((name) => extname(name) === ".js");
  if (names.length !== 1) throw new Error(`expected one JavaScript artifact in ${directory}, found ${names.join(", ")}`);
  return join(directory, names[0]);
}

async function viteLane(entry, outputDirectory, minify, terserOptions) {
  await viteBuild({
    configFile: false,
    logLevel: "silent",
    build: {
      lib: { entry, formats: ["es"], fileName: "app" },
      outDir: outputDirectory,
      emptyOutDir: true,
      minify,
      terserOptions,
      target: "baseline-widely-available",
    },
  });
  return onlyJavaScript(outputDirectory);
}

async function rawBundle(entry, output) {
  const result = await esbuild({
    absWorkingDir: labRoot,
    bundle: true,
    entryPoints: [entry],
    format: "esm",
    legalComments: "none",
    minify: false,
    platform: "browser",
    target: "es2021",
    write: false,
  });
  await writeFile(output, result.outputFiles[0].contents);
  return output;
}

function artifact(id, label, tool, mode, properties, path, contents) {
  return {
    id,
    label,
    tool,
    mode,
    identifierMangling: mode !== "unmangled",
    propertyMangling: properties,
    output: relative(repoRoot, path),
    ...metrics(contents),
  };
}

await rm(buildRoot, { recursive: true, force: true });
await mkdir(buildRoot, { recursive: true });
command(cargo, ["build", "--release", "--bin", "lilscript"], { cwd: repoRoot });

const results = [];
for (const spec of cases) {
  const root = join(buildRoot, spec.id);
  await mkdir(root, { recursive: true });
  const jsEntry = join(labRoot, "apps", spec.id, "js/main.js");
  const lilEntry = join(labRoot, "apps", spec.id, "lil/main.lil");
  const host = spec.host ? join(labRoot, spec.host) : null;
  const rawPath = await rawBundle(jsEntry, join(root, "npm-readable.mjs"));
  const expected = execute(rawPath, host);

  const lanes = [];
  for (const lane of [
    { id: "vite-unminified", label: "Vite 8 / Rolldown, minify false", minify: false, mode: "unmangled", properties: "off" },
    { id: "vite-oxc", label: "Vite 8 / Oxc default", minify: "oxc", mode: "identifier", properties: "off" },
    {
      id: "vite-terser-properties",
      label: "Vite 8 / Terser, `_` properties",
      minify: "terser",
      mode: "property",
      properties: "private-prefix",
      terserOptions: { compress: { passes: 3 }, mangle: { properties: { regex: /^_/ } } },
    },
  ]) {
    const path = await viteLane(jsEntry, join(root, lane.id), lane.minify, lane.terserOptions);
    const contents = await readFile(path);
    lanes.push(artifact(lane.id, lane.label, "Vite 8", lane.mode, lane.properties, path, contents));
  }

  const closurePath = join(root, "closure-advanced.mjs");
  command(closure, [
    "--js", rawPath,
    "--js_output_file", closurePath,
    "--compilation_level", "ADVANCED",
    "--language_in", "ECMASCRIPT_2021",
    "--language_out", "ECMASCRIPT_2021",
    "--warning_level", "QUIET",
    "--emit_use_strict=false",
    "--rewrite_polyfills=false",
    "--externs", join(labRoot, "closure.externs.js"),
  ]);
  lanes.push(artifact(
    "closure-advanced",
    "Closure Compiler ADVANCED / closed app",
    "Closure Compiler",
    "property",
    "closed-world",
    closurePath,
    await readFile(closurePath),
  ));

  for (const lane of [
    { id: "lilscript-unmangled", label: "LilScript / optimization and mangling off", config: "unmangled.toml", mode: "unmangled", properties: "off" },
    { id: "lilscript-public-safe", label: "LilScript / public-safe identifiers", config: "public-safe.toml", mode: "identifier", properties: "off" },
    { id: "lilscript-closed-world", label: "LilScript / closed-world properties", config: "closed-world.toml", mode: "property", properties: "closed-world" },
  ]) {
    const path = join(root, `${lane.id}.mjs`);
    command(compiler, [lilEntry, "--target", "js", "--config", join(labRoot, "config", lane.config), "-o", path]);
    lanes.push(artifact(lane.id, lane.label, "LilScript", lane.mode, lane.properties, path, await readFile(path)));
  }

  let nativeObserved = null;
  if (spec.native !== false) {
    const nativeBase = join(root, "native-public-safe");
    command(compiler, [lilEntry, "--target", "all", "--config", join(labRoot, "config/unmangled.toml"), "-o", nativeBase]);
    nativeObserved = normalize(command(nativeBase, []));
  }

  const lilClosedPath = join(root, "lilscript-closed-world.mjs");
  const lilVitePath = await viteLane(lilClosedPath, join(root, "lilscript-vite-oxc"), "oxc");
  lanes.push(artifact(
    "lilscript-vite-oxc",
    "LilScript closed world + Vite 8 / Oxc",
    "LilScript + Vite 8",
    "property",
    "closed-world",
    lilVitePath,
    await readFile(lilVitePath),
  ));

  const observations = Object.fromEntries(lanes.map((lane) => [lane.id, execute(join(repoRoot, lane.output), host)]));
  if (nativeObserved != null) observations.native = nativeObserved;
  for (const [lane, observed] of Object.entries(observations)) {
    if (observed !== expected) throw new Error(`${spec.id}/${lane} mismatch\nexpected ${expected}\nobserved ${observed}`);
  }

  results.push({
    ...spec,
    expected,
    category: spec.category ?? "real-app",
    verification: { lanes: Object.keys(observations).length, native: nativeObserved != null, cEmitted: nativeObserved != null },
    source: {
      javascript: relative(repoRoot, jsEntry),
      lilscript: relative(repoRoot, lilEntry),
    },
    artifacts: lanes,
  });
}

const packageJson = JSON.parse(readFileSync(join(labRoot, "package.json"), "utf8"));
const report = {
  metadata: {
    generatedAt: new Date().toISOString(),
    compilerRevision: command("git", ["rev-parse", "--short", "HEAD"], { cwd: repoRoot }),
    node: process.version,
    vite: packageJson.devDependencies.vite,
    terser: packageJson.devDependencies.terser,
    closure: packageJson.devDependencies["google-closure-compiler"],
    system: `${platform()} ${release()} ${arch()}`,
    cpu: cpus()[0]?.model ?? "unknown",
    codecs: { gzip: 9, brotli: 11 },
  },
  fairness: "Same npm versions, selected APIs, source-level workload, closed-world boundary, expected stdout, JavaScript target, and codec settings. Property-mangled lanes may rename only fields that never cross the printed app contract.",
  results,
};
const markdown = [
  "# Real-application and mangling results",
  "",
  `Generated ${report.metadata.generatedAt} with Node ${report.metadata.node}, Vite ${report.metadata.vite}, Terser ${report.metadata.terser}, and Closure Compiler ${report.metadata.closure}.`,
  "",
  "Every JavaScript lane matches the fixed contract before measurement. Application scenarios also match C/native output. Raw, gzip-9, and Brotli-11 are independent byte measurements. Compare rows within one project only.",
];
for (const result of report.results) {
  markdown.push(
    "",
    `## ${result.title}`,
    "",
    result.id === "property-ledger" ? "This focused host-boundary stress contract is not an npm or application claim. The host observes values but not keys, so property renaming is legal while scalar replacement is not.\n" : "",
    `Contract: \`${result.expected}\``,
    "",
    "| Lane | Raw | Gzip-9 | Brotli-11 | Properties |",
    "| --- | ---: | ---: | ---: | --- |",
    ...result.artifacts.map((item) => `| ${item.label} | ${item.raw} | ${item.gzip} | ${item.brotli} | ${item.propertyMangling} |`),
  );
}
await writeFile(join(buildRoot, "results.json"), `${JSON.stringify(report, null, 2)}\n`);
await writeFile(webResults, `${JSON.stringify(report, null, 2)}\n`);
await writeFile(join(labRoot, "RESULTS.md"), `${markdown.join("\n")}\n`);
command(process.execPath, [join(repoRoot, "web/scripts/generate-benchmark-catalog.mjs")], { cwd: repoRoot });
console.log(`Verified ${results.length} real-app scenarios across ${results[0].artifacts.length} JavaScript lanes plus native execution.`);
