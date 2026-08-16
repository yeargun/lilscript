import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve } from "node:path";
import { arch, cpus, homedir, platform, release } from "node:os";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import { build as viteBuild } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";
import { prepareScenarioToolchain } from "./toolchain.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const buildRoot = join(labRoot, "build");
const cargo = process.env.CARGO ?? join(homedir(), ".cargo/bin/cargo");
const closure = join(labRoot, "node_modules/.bin/google-closure-compiler");
const webResults = join(repoRoot, "web/src/scenario-results.json");
const cases = [
  {
    id: "login-risk",
    title: "Login risk scoring",
    summary:
      "A login audit combines edit distance, two production hash algorithms, clamping, private result records, and deterministic batching.",
    packages: ["js-levenshtein", "string-hash", "@emotion/hash", "clamp"],
  },
  {
    id: "animation-timeline",
    title: "Animation timeline",
    summary:
      "A frame scheduler combines cubic-bezier easing, steps, interpolation, clamping, and private frame records.",
    packages: ["@motionone/easing", "clamp", "lerp"],
  },
  {
    id: "geometry-hit-test",
    title: "Geometry hit testing",
    summary:
      "A drawing-style hit-test batch combines adaptive robust predicates, confidence clamping, and private hit records.",
    packages: ["robust-predicates", "clamp"],
  },
  {
    id: "property-ledger",
    title: "Closed property ledger",
    summary:
      "A focused host-boundary contract keeps aggregate objects alive while declaring their field names unobservable, making private-property mangling measurable instead of optimized away.",
    packages: [],
    category: "mangling",
    host: "apps/property-ledger/host.mjs",
    native: false,
  },
];
const sha256File = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

function command(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? labRoot,
    encoding: "utf8",
    timeout: options.timeout ?? 300_000,
    maxBuffer: 64 * 1024 * 1024,
    env: options.env ?? process.env,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")} failed (${result.status})\n${result.stdout}${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

function normalize(value) {
  return value.replaceAll("\r\n", "\n").trimEnd();
}

function execute(path, host) {
  return normalize(
    command(process.execPath, host ? ["--import", host, path] : [path]),
  );
}

async function onlyJavaScript(directory) {
  const names = (await readdir(directory)).filter(
    (name) => extname(name) === ".js",
  );
  if (names.length !== 1)
    throw new Error(
      `expected one JavaScript artifact in ${directory}, found ${names.join(", ")}`,
    );
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

function artifact(id, label, tool, mode, properties, path) {
  return {
    id,
    label,
    tool,
    mode,
    identifierMangling: mode !== "unmangled",
    propertyMangling: properties,
    output: relative(repoRoot, path),
    ...canonicalCodecSizesForFile(path, `${id} scenario artifact`),
  };
}

const toolchain = prepareScenarioToolchain({
  repoRoot,
  cargo,
  command,
});
const { compiler } = toolchain;
requireCanonicalCodecRuntime("scenario publication measurement");
await rm(buildRoot, { recursive: true, force: true });
await mkdir(buildRoot, { recursive: true });

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
    {
      id: "vite-unminified",
      label: "Vite 8 / Rolldown, minify false",
      minify: false,
      mode: "unmangled",
      properties: "off",
    },
    {
      id: "vite-oxc",
      label: "Vite 8 / Oxc default",
      minify: "oxc",
      mode: "identifier",
      properties: "off",
    },
    {
      id: "vite-terser-properties",
      label: "Vite 8 / Terser, `_` properties",
      minify: "terser",
      mode: "property",
      properties: "private-prefix",
      terserOptions: {
        compress: { passes: 3 },
        mangle: { properties: { regex: /^_/ } },
      },
    },
  ]) {
    const path = await viteLane(
      jsEntry,
      join(root, lane.id),
      lane.minify,
      lane.terserOptions,
    );
    lanes.push(
      artifact(lane.id, lane.label, "Vite 8", lane.mode, lane.properties, path),
    );
  }

  const closurePath = join(root, "closure-advanced.mjs");
  command(closure, [
    "--js",
    rawPath,
    "--js_output_file",
    closurePath,
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
    "--externs",
    join(labRoot, "closure.externs.js"),
  ]);
  lanes.push(
    artifact(
      "closure-advanced",
      "Closure Compiler ADVANCED / closed app",
      "Closure Compiler",
      "property",
      "closed-world",
      closurePath,
    ),
  );

  for (const lane of [
    {
      id: "lilscript-unmangled",
      label: "LilScript / optimization and mangling off",
      config: "unmangled.toml",
      mode: "unmangled",
      properties: "off",
    },
    {
      id: "lilscript-public-safe",
      label: "LilScript / public-safe identifiers",
      config: "public-safe.toml",
      mode: "identifier",
      properties: "off",
    },
    {
      id: "lilscript-closed-world",
      label: "LilScript / closed-world properties",
      config: "closed-world.toml",
      mode: "property",
      properties: "closed-world",
    },
  ]) {
    const path = join(root, `${lane.id}.mjs`);
    command(compiler, [
      lilEntry,
      "--target",
      "js",
      "--config",
      join(labRoot, "config", lane.config),
      "-o",
      path,
    ]);
    lanes.push(
      artifact(
        lane.id,
        lane.label,
        "LilScript",
        lane.mode,
        lane.properties,
        path,
      ),
    );
  }

  let nativeObserved = null;
  if (spec.native !== false) {
    const nativeBase = join(root, "native-public-safe");
    command(compiler, [
      lilEntry,
      "--target",
      "all",
      "--config",
      join(labRoot, "config/unmangled.toml"),
      "-o",
      nativeBase,
    ]);
    nativeObserved = normalize(command(nativeBase, []));
  }

  const lilClosedPath = join(root, "lilscript-closed-world.mjs");
  const lilVitePath = await viteLane(
    lilClosedPath,
    join(root, "lilscript-vite-oxc"),
    "oxc",
  );
  lanes.push(
    artifact(
      "lilscript-vite-oxc",
      "LilScript closed world + Vite 8 / Oxc",
      "LilScript + Vite 8",
      "property",
      "closed-world",
      lilVitePath,
    ),
  );

  const observations = Object.fromEntries(
    lanes.map((lane) => [lane.id, execute(join(repoRoot, lane.output), host)]),
  );
  if (nativeObserved != null) observations.native = nativeObserved;
  for (const [lane, observed] of Object.entries(observations)) {
    if (observed !== expected)
      throw new Error(
        `${spec.id}/${lane} mismatch\nexpected ${expected}\nobserved ${observed}`,
      );
  }

  results.push({
    ...spec,
    expected,
    category: spec.category ?? "real-app",
    verification: {
      lanes: Object.keys(observations).length,
      native: nativeObserved != null,
      cEmitted: nativeObserved != null,
    },
    source: {
      javascript: relative(repoRoot, jsEntry),
      lilscript: relative(repoRoot, lilEntry),
    },
    artifacts: lanes,
  });
}

const packageJson = JSON.parse(
  readFileSync(join(labRoot, "package.json"), "utf8"),
);
const report = {
  schemaVersion: 2,
  metadata: {
    generatedAt: new Date().toISOString(),
    compilerRevision: command("git", ["rev-parse", "--short", "HEAD"], {
      cwd: repoRoot,
    }),
    toolchainSource: toolchain.toolchainSource,
    compiler: {
      path: relative(repoRoot, compiler),
      version: command(compiler, ["--version"], { cwd: repoRoot }),
      sha256: sha256File(compiler),
    },
    configs: Object.fromEntries(
      ["unmangled", "public-safe", "closed-world"].map((name) => {
        const path = join(labRoot, `config/${name}.toml`);
        return [
          name,
          { path: relative(repoRoot, path), sha256: sha256File(path) },
        ];
      }),
    ),
    node: process.version,
    vite: packageJson.devDependencies.vite,
    terser: packageJson.devDependencies.terser,
    closure: packageJson.devDependencies["google-closure-compiler"],
    system: `${platform()} ${release()} ${arch()}`,
    cpu: cpus()[0]?.model ?? "unknown",
    codecs: canonicalCodecProvenance("scenario report"),
    objectiveContract: {
      artifactObjective: "brotli",
      hardSizeGate: null,
      study: "configuration-and-mangling-diagnostic",
      crossMetricsAreDiagnostic: ["raw", "gzip"],
    },
  },
  fairness:
    "Same npm versions, selected APIs, source-level workload, closed-world boundary, expected stdout, JavaScript target, and codec settings. Property-mangled lanes may rename only fields that never cross the printed app contract.",
  results,
};
const markdown = [
  "# Real-application and mangling results",
  "",
  `Generated ${report.metadata.generatedAt} with Node ${report.metadata.node}, Vite ${report.metadata.vite}, Terser ${report.metadata.terser}, and Closure Compiler ${report.metadata.closure}.`,
  "",
  "Every JavaScript lane matches the fixed contract before measurement. Application scenarios also match C/native output. This is a configuration/mangling study: its LilScript configs are Brotli-oriented, while raw and gzip are diagnostic cross-metrics that may regress. It is not a three-objective language-superiority gate. Compare rows within one project only.",
];
for (const result of report.results) {
  markdown.push(
    "",
    `## ${result.title}`,
    "",
    result.id === "property-ledger"
      ? "This focused host-boundary stress contract is not an npm or application claim. The host observes values but not keys, so property renaming is legal while scalar replacement is not.\n"
      : "",
    `Contract: \`${result.expected}\``,
    "",
    "| Lane | Raw | Gzip-9 | Brotli-11 | Properties |",
    "| --- | ---: | ---: | ---: | --- |",
    ...result.artifacts.map(
      (item) =>
        `| ${item.label} | ${item.raw} | ${item.gzip} | ${item.brotli} | ${item.propertyMangling} |`,
    ),
  );
}
await writeFile(
  join(buildRoot, "results.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
await writeFile(webResults, `${JSON.stringify(report, null, 2)}\n`);
await writeFile(join(labRoot, "RESULTS.md"), `${markdown.join("\n")}\n`);
console.log(
  `Verified ${results.length} real-app scenarios across ${results[0].artifacts.length} JavaScript lanes plus native execution.`,
);
