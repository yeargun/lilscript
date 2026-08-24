import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  writeFileSync,
  readFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
  requireCanonicalCodecRuntime,
} from "../../benchmarks/codec-contract.mjs";
import { catalog } from "./catalog.mjs";
import {
  assertNoBehaviorLabelSplits,
  MIN_UNIQUE_GENERATED_BEHAVIORS,
  summarizeBehaviorCoverage,
} from "./coverage.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const popular = join(repo, "benchmarks/popular");
const require = createRequire(join(popular, "package.json"));
const compilerOverride = process.env.LILSCRIPT;
const codecOverride = process.env.LILSCRIPT_CODEC;
if (Boolean(compilerOverride) !== Boolean(codecOverride)) {
  throw new Error(
    "LILSCRIPT and LILSCRIPT_CODEC overrides must be supplied together so a report cannot mix unrelated builds",
  );
}
const compiler = compilerOverride
  ? resolve(process.cwd(), compilerOverride)
  : join(repo, "target/release/lilscript");
const generatedRoot = join(here, "generated");
const buildRoot = join(here, "build");
const oraclePath = join(here, "oracle-manifest.json");
const metrics = ["raw", "gzip9", "brotli11"];
const javascriptTarget = "es2022";
const baselineToolNames = [
  "terser",
  "terser-properties",
  "oxc",
  "esbuild-script",
  "esbuild-iife",
];
const baselineOptions = {
  terser: {
    ecma: 2022,
    compress: {
      ecma: 2022,
      passes: 3,
      drop_console: false,
      toplevel: true,
    },
    mangle: { toplevel: true },
    format: { ecma: 2022, comments: false },
  },
  "terser-properties": {
    ecma: 2022,
    compress: {
      ecma: 2022,
      passes: 3,
      drop_console: false,
      toplevel: true,
    },
    mangle: {
      toplevel: true,
      properties: {
        builtins: false,
        keep_quoted: true,
        reserved: ["__proto__", "constructor", "prototype"],
      },
    },
    format: { ecma: 2022, comments: false },
  },
  oxc: {
    module: false,
    compress: { target: javascriptTarget },
    mangle: { toplevel: true },
    codegen: { target: javascriptTarget, legalComments: "none" },
  },
  "esbuild-script": {
    minify: true,
    target: javascriptTarget,
    legalComments: "none",
  },
  "esbuild-iife": {
    minify: true,
    target: javascriptTarget,
    legalComments: "none",
    format: "iife",
  },
};
const lanes = [
  { name: "raw", metric: "raw", config: join(here, "configs/raw.toml") },
  { name: "gzip", metric: "gzip9", config: join(here, "configs/gzip.toml") },
  {
    name: "brotli",
    metric: "brotli11",
    config: join(here, "configs/brotli.toml"),
  },
];

function requireSupportedNode() {
  const [major, minor] = process.versions.node.split(".").map(Number);
  const supported =
    (major === 20 && minor >= 19) ||
    (major === 22 && minor >= 12) ||
    major > 22;
  if (!supported) {
    throw new Error(
      `comparison/cases requires Node 20.19+ or 22.12+; found ${process.versions.node}. Run nvm use.`,
    );
  }
}

requireSupportedNode();

const argv = process.argv.slice(2);
const onlyIndex = argv.indexOf("--only");
const only = onlyIndex === -1 ? null : argv[onlyIndex + 1];
const updateOracles = argv.includes("--update-oracles");
const canonicalOnly = argv.includes("--canonical-only");
const knownArguments = new Set(["--only", "--update-oracles", "--canonical-only"]);
for (const [index, argument] of argv.entries()) {
  if (index === onlyIndex + 1) continue;
  if (!knownArguments.has(argument)) {
    throw new Error(`unknown argument: ${argument}`);
  }
}
if (onlyIndex !== -1 && !only) {
  throw new Error("--only requires a non-empty substring");
}

function artifactStem(name) {
  return name.replaceAll("/", "--");
}

function compareCodeUnits(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function parseCaseToml(text, label) {
  const out = { terserProperties: true, terserPropertyReason: null };
  const seen = new Set();
  for (const [index, raw] of text.split("\n").entries()) {
    const line = raw.replace(/#.*$/, "").trim();
    if (!line) continue;
    const match = line.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$/);
    if (!match) {
      throw new Error(`${label}:${index + 1}: invalid case metadata`);
    }
    const [, key, rawValue] = match;
    if (seen.has(key)) {
      throw new Error(`${label}:${index + 1}: duplicate ${key}`);
    }
    seen.add(key);
    const value = rawValue.trim();
    if (key === "expect") {
      const parsed = value.match(/^(?:"(le|lt)"|'(le|lt)')$/);
      if (!parsed) {
        throw new Error(`${label}:${index + 1}: expect must be "le" or "lt"`);
      }
      out.expect = parsed[1] ?? parsed[2];
      continue;
    }
    if (key === "terser_properties") {
      if (value !== "true" && value !== "false") {
        throw new Error(
          `${label}:${index + 1}: terser_properties must be true or false`,
        );
      }
      out.terserProperties = value === "true";
      continue;
    }
    if (key === "terser_property_reason") {
      const parsed = value.match(/^(?:"([^"]+)"|'([^']+)')$/);
      if (!parsed) {
        throw new Error(
          `${label}:${index + 1}: terser_property_reason must be a non-empty quoted string`,
        );
      }
      out.terserPropertyReason = parsed[1] ?? parsed[2];
      continue;
    }
    throw new Error(`${label}:${index + 1}: unknown case metadata key ${key}`);
  }
  if (!out.expect) {
    throw new Error(`${label}: missing required expect metadata`);
  }
  if (!out.terserProperties && !out.terserPropertyReason) {
    throw new Error(
      `${label}: terser_properties=false requires terser_property_reason`,
    );
  }
  if (out.terserProperties && out.terserPropertyReason) {
    throw new Error(
      `${label}: terser_property_reason is only valid when terser_properties=false`,
    );
  }
  return out;
}

function loadCanonicalCases() {
  const root = join(here, "canonical");
  if (!existsSync(root)) {
    return [];
  }
  const cases = [];
  const walk = (dir) => {
    for (const ent of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, ent.name);
      if (ent.isDirectory()) {
        walk(path);
        continue;
      }
      if (ent.name !== "case.toml") continue;
      const folder = dirname(path);
      const meta = parseCaseToml(readFileSync(path, "utf8"), path);
      const lilPath = existsSync(join(folder, "main.lil"))
        ? join(folder, "main.lil")
        : join(folder, "lilscript", "main.lil");
      const jsPath = existsSync(join(folder, "main.js"))
        ? join(folder, "main.js")
        : join(folder, "javascript", "main.js");
      if (!existsSync(lilPath) || !existsSync(jsPath)) {
        throw new Error(`canonical case ${folder} needs main.lil and main.js`);
      }
      const rel = relative(root, folder).replaceAll("\\", "/");
      cases.push({
        name: rel,
        behavior: `canonical/${rel}`,
        expect: meta.expect,
        terserProperties: meta.terserProperties,
        terserPropertyReason: meta.terserPropertyReason,
        lil: readFileSync(lilPath, "utf8"),
        js: readFileSync(jsPath, "utf8"),
        origin: "canonical",
      });
    }
  };
  walk(root);
  cases.sort((left, right) => compareCodeUnits(left.name, right.name));
  return cases;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function reportSizes(measurement) {
  return {
    raw: measurement.raw,
    gzip9: measurement.gzip,
    brotli11: measurement.brotli,
  };
}

function run(program, args, cwd = here, timeout = 10 * 60 * 1000) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    timeout,
  });
  if (result.status !== 0) {
    const spawnFailure = result.error ? `${result.error.message}\n` : "";
    throw new Error(
      `${program} ${args.join(" ")}\n${spawnFailure}${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout;
}

function execute(source) {
  const result = spawnSync(process.execPath, ["--input-type=commonjs"], {
    cwd: here,
    encoding: "utf8",
    input: source,
    timeout: 10_000,
  });
  if (result.status !== 0) {
    throw new Error(
      `execute failed${result.signal ? ` (${result.signal})` : ""}\n` +
        `${result.error?.message ?? ""}\n${result.stdout ?? ""}${result.stderr ?? ""}\n${source}`,
    );
  }
  return result.stdout;
}

function prepareCompiler() {
  if (!compilerOverride) {
    const cargo = process.env.CARGO ?? "cargo";
    try {
      run(
        cargo,
        [
          "build",
          "--manifest-path",
          join(repo, "Cargo.toml"),
          "--release",
          "--bin",
          "lilscript",
          "--bin",
          "lilscript-codec",
        ],
        repo,
      );
    } catch (error) {
      throw new Error(
        `failed to build a fresh release compiler with CARGO=${cargo}\n` +
          `Set CARGO to the Cargo executable, or set LILSCRIPT to an explicit compiler binary.\n${error.message}`,
      );
    }
  } else if (!existsSync(compiler)) {
    throw new Error(`LILSCRIPT does not exist: ${compiler}`);
  } else if (!existsSync(resolve(process.cwd(), codecOverride))) {
    throw new Error(`LILSCRIPT_CODEC does not exist: ${codecOverride}`);
  }

  const probe = spawnSync(compiler, ["--help"], { encoding: "utf8" });
  if (probe.status !== 0) {
    throw new Error(
      `compiler is not runnable: ${compiler}\n${probe.error?.message ?? ""}${probe.stderr ?? ""}`,
    );
  }
  const version = spawnSync(compiler, ["--version"], { encoding: "utf8" });
  return {
    source: compilerOverride ? "LILSCRIPT" : "cargo-build-release",
    path: compiler,
    version: version.status === 0 ? version.stdout.trim() : "unreported",
    digest: sha256(readFileSync(compiler)),
  };
}

async function loadMinifiers() {
  const installHint = `Run npm ci --prefix ${popular}`;
  try {
    const { minify: terserMinify } = require("terser");
    const esbuild = require("esbuild");
    const { minifySync: oxcMinify } = await import(
      pathToFileURL(join(popular, "node_modules/rolldown/dist/utils-index.mjs"))
    );
    return { terserMinify, esbuild, oxcMinify };
  } catch (error) {
    throw new Error(
      `comparison minifier dependencies are unavailable. ${installHint}\n${error.message}`,
    );
  }
}

async function minifyJavaScript(entry, minifiers) {
  const { js: source, name } = entry;
  const { terserMinify, esbuild, oxcMinify } = minifiers;
  let started = process.hrtime.bigint();
  const terser = await terserMinify(
    source,
    structuredClone(baselineOptions.terser),
  );
  const terserDurationMs =
    Number(process.hrtime.bigint() - started) / 1_000_000;
  if (!terser.code) {
    throw new Error(`terser failed: ${terser.error}`);
  }
  let terserProperties = null;
  let terserPropertiesDurationMs = null;
  if (entry.terserProperties) {
    started = process.hrtime.bigint();
    terserProperties = await terserMinify(
      source,
      structuredClone(baselineOptions["terser-properties"]),
    );
    terserPropertiesDurationMs =
      Number(process.hrtime.bigint() - started) / 1_000_000;
    if (!terserProperties.code) {
      throw new Error(
        `terser property mangling failed: ${terserProperties.error}`,
      );
    }
  }
  started = process.hrtime.bigint();
  const oxc = oxcMinify(
    `${name}.js`,
    source,
    structuredClone(baselineOptions.oxc),
  );
  const oxcDurationMs = Number(process.hrtime.bigint() - started) / 1_000_000;
  if (oxc.errors?.length) {
    throw new Error(`oxc failed: ${JSON.stringify(oxc.errors)}`);
  }
  started = process.hrtime.bigint();
  const esbuildScript = await esbuild.transform(
    source,
    structuredClone(baselineOptions["esbuild-script"]),
  );
  const esbuildScriptDurationMs =
    Number(process.hrtime.bigint() - started) / 1_000_000;
  started = process.hrtime.bigint();
  const esbuildIife = await esbuild.transform(
    source,
    structuredClone(baselineOptions["esbuild-iife"]),
  );
  const esbuildIifeDurationMs =
    Number(process.hrtime.bigint() - started) / 1_000_000;
  const candidates = [
    { tool: "terser", code: terser.code, durationMs: terserDurationMs },
    { tool: "oxc", code: oxc.code, durationMs: oxcDurationMs },
    {
      tool: "esbuild-script",
      code: esbuildScript.code,
      durationMs: esbuildScriptDurationMs,
    },
    {
      tool: "esbuild-iife",
      code: esbuildIife.code,
      durationMs: esbuildIifeDurationMs,
    },
  ];
  if (terserProperties) {
    candidates.splice(1, 0, {
      tool: "terser-properties",
      code: terserProperties.code,
      durationMs: terserPropertiesDurationMs,
    });
  }
  return candidates.map((candidate) => ({
    ...candidate,
    digest: sha256(candidate.code),
  }));
}

function bestForMetric(candidates, metric) {
  return [...candidates].sort(
    (left, right) =>
      left.sizes[metric] - right.sizes[metric] ||
      left.sizes.raw - right.sizes.raw ||
      compareCodeUnits(left.tool, right.tool) ||
      compareCodeUnits(left.code, right.code),
  )[0];
}

function writeCase(entry, expected) {
  const dir = join(generatedRoot, entry.name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "main.lil"), entry.lil);
  writeFileSync(join(dir, "main.js"), entry.js);
  writeFileSync(join(dir, "expected.txt"), expected);
  writeFileSync(
    join(dir, "case.json"),
    `${JSON.stringify(
      {
        name: entry.name,
        behavior: entry.behavior,
        expect: entry.expect,
        terserProperties: entry.terserProperties,
        terserPropertyReason: entry.terserPropertyReason,
      },
      null,
      2,
    )}\n`,
  );
  return dir;
}

function rmrf(path) {
  for (let attempt = 0; attempt < 6; attempt++) {
    try {
      rmSync(path, { recursive: true, force: true });
      return;
    } catch (error) {
      if (attempt === 5) throw error;
    }
  }
}

function gate(actual, baseline, expectation) {
  return expectation === "lt" ? actual < baseline : actual <= baseline;
}

function toolVersions(compilerProvenance) {
  const rolldownPackage = require("rolldown/package.json");
  const binding = Object.keys(rolldownPackage.optionalDependencies ?? {})
    .filter((name) => name.startsWith("@rolldown/binding-"))
    .map((name) => ({
      name,
      packagePath: join(
        popular,
        "node_modules",
        ...name.split("/"),
        "package.json",
      ),
    }))
    .find(({ packagePath }) => existsSync(packagePath));
  return {
    node: process.versions.node,
    lilscript: compilerProvenance,
    terser: require("terser/package.json").version,
    oxcViaRolldown: {
      rolldown: rolldownPackage.version,
      binding: binding
        ? {
            name: binding.name,
            version: JSON.parse(readFileSync(binding.packagePath, "utf8"))
              .version,
          }
        : null,
    },
    esbuild: require("esbuild/package.json").version,
  };
}

function corpusDigest(entries) {
  return sha256(
    JSON.stringify(
      entries.map((entry) => ({
        name: entry.name,
        behavior: entry.behavior,
        terserProperties: entry.terserProperties,
        terserPropertyReason: entry.terserPropertyReason,
        lilscript: entry.lil,
        javascript: entry.js,
        expect: entry.expect,
      })),
    ),
  );
}

function withoutDiagnosticTimings(value) {
  if (Array.isArray(value)) {
    return value.map(withoutDiagnosticTimings);
  }
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key]) => key !== "durationMs" && key !== "compileDurationMs",
      )
      .map(([key, child]) => [key, withoutDiagnosticTimings(child)]),
  );
}

function evaluateCatalog(entries) {
  const names = new Set();
  const expectedByName = new Map();
  const oracleRecords = [];
  for (const entry of entries) {
    if (!entry || typeof entry.name !== "string" || entry.name.length === 0) {
      throw new Error("every catalog entry must have a non-empty name");
    }
    if (names.has(entry.name)) {
      throw new Error(`duplicate catalog case: ${entry.name}`);
    }
    names.add(entry.name);
    if (entry.expect !== "le" && entry.expect !== "lt") {
      throw new Error(`${entry.name}: expect must be \"le\" or \"lt\"`);
    }
    if (typeof entry.terserProperties !== "boolean") {
      throw new Error(`${entry.name}: terserProperties must be boolean`);
    }
    if (
      !entry.terserProperties &&
      (typeof entry.terserPropertyReason !== "string" ||
        entry.terserPropertyReason.length === 0)
    ) {
      throw new Error(
        `${entry.name}: property-mangling opt-outs require a reason`,
      );
    }
    if (entry.terserProperties && entry.terserPropertyReason !== null) {
      throw new Error(
        `${entry.name}: property-mangling reasons are only valid for opt-outs`,
      );
    }
    let expected;
    try {
      expected = execute(entry.js);
    } catch (error) {
      throw new Error(
        `${entry.name}: reference JavaScript failed\n${error.message}`,
      );
    }
    expectedByName.set(entry.name, expected);
    oracleRecords.push({
      name: entry.name,
      behavior: entry.behavior,
      terserProperties: entry.terserProperties,
      terserPropertyReason: entry.terserPropertyReason,
      expect: entry.expect,
      javascript: entry.js,
      stdout: expected,
    });
  }
  return {
    expectedByName,
    manifest: {
      schemaVersion: 3,
      algorithm: "sha256",
      cases: entries.length,
      digest: sha256(JSON.stringify(oracleRecords)),
    },
    corpusDigest: corpusDigest(entries),
  };
}

const catalogEntries = catalog();
if (catalogEntries.length === 0) {
  throw new Error("catalog is empty");
}
const canonicalEntries = loadCanonicalCases();
const generatedBehaviorLabelAudit = assertNoBehaviorLabelSplits(catalogEntries, {
  label: "generated catalog",
});
const catalogCoverage = summarizeBehaviorCoverage(catalogEntries, {
  label: "generated catalog",
  minimumUniqueBehaviors: MIN_UNIQUE_GENERATED_BEHAVIORS,
});
const canonicalCoverage = summarizeBehaviorCoverage(canonicalEntries, {
  label: "canonical corpus",
});
const fullCoverage = summarizeBehaviorCoverage(
  [...catalogEntries, ...canonicalEntries],
  { label: "complete micro corpus" },
);
const catalogNames = new Set(catalogEntries.map((entry) => entry.name));
for (const entry of canonicalEntries) {
  if (catalogNames.has(entry.name)) {
    throw new Error(`canonical case name collides with catalog: ${entry.name}`);
  }
}
const catalogEvaluation = evaluateCatalog(catalogEntries);
const completeCorpusDigest = corpusDigest([
  ...catalogEntries,
  ...canonicalEntries,
]);
if (updateOracles) {
  if (only) {
    throw new Error("--update-oracles cannot be combined with --only");
  }
  if (canonicalOnly) {
    throw new Error("--update-oracles cannot be combined with --canonical-only");
  }
  writeFileSync(
    oraclePath,
    `${JSON.stringify(catalogEvaluation.manifest, null, 2)}\n`,
  );
  console.log(
    `Updated ${oraclePath} for ${catalogEvaluation.manifest.cases} cases (${catalogEvaluation.manifest.digest}).`,
  );
  process.exit(0);
}

let checkedInOracle;
try {
  checkedInOracle = JSON.parse(readFileSync(oraclePath, "utf8"));
} catch (error) {
  throw new Error(
    `cannot read ${oraclePath}\n${error.message}\n` +
      "Review the reference programs, then run node comparison/cases/run.mjs --update-oracles.",
  );
}
if (
  checkedInOracle.schemaVersion !== catalogEvaluation.manifest.schemaVersion ||
  checkedInOracle.algorithm !== catalogEvaluation.manifest.algorithm ||
  checkedInOracle.cases !== catalogEvaluation.manifest.cases ||
  checkedInOracle.digest !== catalogEvaluation.manifest.digest
) {
  throw new Error(
    `reference oracle drifted: expected ${checkedInOracle.digest ?? "<missing>"}, ` +
      `observed ${catalogEvaluation.manifest.digest}\n` +
      "Review the JavaScript source and stdout changes, then run node comparison/cases/run.mjs --update-oracles.",
  );
}

const allEntries = canonicalOnly
  ? canonicalEntries
  : [...catalogEntries, ...canonicalEntries];
if (canonicalOnly && canonicalEntries.length === 0) {
  throw new Error("no canonical cases under comparison/cases/canonical");
}

const selected = allEntries.filter((entry) =>
  only ? entry.name.includes(only) : true,
);
if (selected.length === 0) {
  throw new Error(`no cases matched --only ${only}`);
}
for (const entry of selected) {
  if (catalogEvaluation.expectedByName.has(entry.name)) {
    continue;
  }
  try {
    catalogEvaluation.expectedByName.set(entry.name, execute(entry.js));
  } catch (error) {
    throw new Error(`${entry.name}: reference JavaScript failed\n${error.message}`);
  }
}

const minifiers = await loadMinifiers();
const compilerProvenance = prepareCompiler();
requireCanonicalCodecRuntime("comparison/cases hard gate");
rmrf(buildRoot);
mkdirSync(buildRoot, { recursive: true });
rmrf(generatedRoot);
mkdirSync(generatedRoot, { recursive: true });

const failures = [];
const rows = [];
const strictWins = { raw: 0, gzip9: 0, brotli11: 0 };
const nonLosses = { raw: 0, gzip9: 0, brotli11: 0 };
const baselineToolWins = Object.fromEntries(
  metrics.map((metric) => [
    metric,
    Object.fromEntries(baselineToolNames.map((tool) => [tool, 0])),
  ]),
);

function recordFailure(caseFailures, message) {
  failures.push(message);
  caseFailures.push(message);
}

function reportProgress(caseIndex) {
  if ((caseIndex + 1) % 25 === 0 || caseIndex + 1 === selected.length) {
    console.log(`verified ${caseIndex + 1}/${selected.length} cases`);
  }
}

for (const [caseIndex, entry] of selected.entries()) {
  const expected = catalogEvaluation.expectedByName.get(entry.name);
  const dir = writeCase(entry, expected);
  const caseFailures = [];
  let baselines;
  try {
    baselines = await minifyJavaScript(entry, minifiers);
  } catch (error) {
    recordFailure(caseFailures, `${entry.name}: minify\n${error.message}`);
    rows.push({
      name: entry.name,
      behavior: entry.behavior,
      expect: entry.expect,
      terserProperties: entry.terserProperties,
      terserPropertyReason: entry.terserPropertyReason,
      origin: entry.origin ?? "catalog",
      passed: false,
      failures: caseFailures,
      lilscript: {},
      javascript: {},
      baselineCandidates: {},
    });
    reportProgress(caseIndex);
    continue;
  }

  for (const candidate of baselines) {
    const baselineOut = join(
      buildRoot,
      `${artifactStem(entry.name)}.${candidate.tool}.js`,
    );
    writeFileSync(baselineOut, candidate.code);
    candidate.artifactPath = baselineOut.slice(repo.length + 1);
    candidate.absoluteArtifactPath = baselineOut;
  }
  try {
    const measurements = canonicalCodecMeasurementsForFiles(
      baselines.map((candidate) => candidate.absoluteArtifactPath),
      `${entry.name} JavaScript baselines`,
    );
    for (const [index, candidate] of baselines.entries()) {
      candidate.sizes = reportSizes(measurements[index]);
      if (candidate.digest !== measurements[index].sha256) {
        throw new Error(
          `${candidate.tool} artifact digest changed before measurement`,
        );
      }
    }
  } catch (error) {
    recordFailure(
      caseFailures,
      `${entry.name}: canonical baseline measurement\n${error.message}`,
    );
  }

  const validBaselines = [];
  for (const candidate of baselines) {
    try {
      const stdout = execute(candidate.code);
      if (stdout !== expected) {
        candidate.semanticValid = false;
        recordFailure(
          caseFailures,
          `${entry.name}: ${candidate.tool} stdout drifted\nexpected:\n${expected}got:\n${stdout}`,
        );
      } else {
        candidate.semanticValid = true;
        validBaselines.push(candidate);
      }
    } catch (error) {
      candidate.semanticValid = false;
      recordFailure(
        caseFailures,
        `${entry.name}: ${candidate.tool} execution\n${error.message}`,
      );
    }
  }

  if (validBaselines.length === 0) {
    recordFailure(
      caseFailures,
      `${entry.name}: no semantically valid JavaScript baseline`,
    );
  }

  const javascript = Object.fromEntries(
    metrics.map((metric) => {
      const best = bestForMetric(
        validBaselines.filter((candidate) => candidate.sizes),
        metric,
      );
      if (!best) return [metric, null];
      baselineToolWins[metric][best.tool] += 1;
      return [metric, { tool: best.tool, size: best.sizes[metric] }];
    }),
  );
  const lilscript = {};
  const compiledLanes = [];
  for (const lane of lanes) {
    const lilOut = join(
      buildRoot,
      `${artifactStem(entry.name)}.${lane.name}.lil.js`,
    );
    try {
      const compileStarted = process.hrtime.bigint();
      run(
        compiler,
        [
          join(dir, "main.lil"),
          "--config",
          lane.config,
          "--target",
          "js",
          "--mode",
          "production",
          "-o",
          lilOut,
        ],
        here,
        120_000,
      );
      const compileDurationMs =
        Number(process.hrtime.bigint() - compileStarted) / 1_000_000;
      const source = readFileSync(lilOut, "utf8");
      const stdout = execute(source);
      if (stdout !== expected) {
        recordFailure(
          caseFailures,
          `${entry.name}/${lane.name}: LilScript stdout drifted\nexpected:\n${expected}got:\n${stdout}`,
        );
        continue;
      }
      compiledLanes.push({ lane, lilOut, source, compileDurationMs });
    } catch (error) {
      recordFailure(
        caseFailures,
        `${entry.name}/${lane.name}: compile or execute\n${error.message}`,
      );
    }
  }
  try {
    const measurements =
      compiledLanes.length === 0
        ? []
        : canonicalCodecMeasurementsForFiles(
            compiledLanes.map(({ lilOut }) => lilOut),
            `${entry.name} LilScript objective artifacts`,
          );
    for (const [index, compiled] of compiledLanes.entries()) {
      const { lane, lilOut, source, compileDurationMs } = compiled;
      const measurement = measurements[index];
      const measured = reportSizes(measurement);
      const digest = sha256(source);
      if (digest !== measurement.sha256) {
        throw new Error(
          `${lane.name} artifact digest changed before measurement`,
        );
      }
      lilscript[lane.metric] = {
        size: measured[lane.metric],
        artifactSizes: measured,
        digest,
        artifactPath: lilOut.slice(repo.length + 1),
        compileDurationMs,
      };
      const baseline = javascript[lane.metric];
      if (!baseline) {
        continue;
      }
      const ok = gate(measured[lane.metric], baseline.size, entry.expect);
      if (!ok) {
        const operator = entry.expect === "lt" ? ">=" : ">";
        recordFailure(
          caseFailures,
          `${entry.name}/${lane.name}: LilScript ${measured[lane.metric]} ${operator} ${baseline.tool} ${baseline.size}`,
        );
      }
      if (measured[lane.metric] < baseline.size) {
        strictWins[lane.metric] += 1;
      }
      if (measured[lane.metric] <= baseline.size) {
        nonLosses[lane.metric] += 1;
      }
    }
  } catch (error) {
    recordFailure(
      caseFailures,
      `${entry.name}: canonical LilScript measurement\n${error.message}`,
    );
  }
  rows.push({
    name: entry.name,
    behavior: entry.behavior,
    expect: entry.expect,
    terserProperties: entry.terserProperties,
    terserPropertyReason: entry.terserPropertyReason,
    origin: entry.origin ?? "catalog",
    boundary: entry.terserProperties
      ? "closed-world-script"
      : "observable-property-spelling",
    target: javascriptTarget,
    passed: caseFailures.length === 0,
    failures: caseFailures,
    lilscript,
    javascript,
    baselineCandidates: Object.fromEntries(
      baselines.map((candidate) => [
        candidate.tool,
        {
          sizes: candidate.sizes,
          semanticValid: candidate.semanticValid,
          digest: candidate.digest,
          artifactPath: candidate.artifactPath,
          durationMs: candidate.durationMs,
        },
      ]),
    ),
    sourceDigests: {
      lilscript: sha256(entry.lil),
      javascript: sha256(entry.js),
      expectedStdout: sha256(expected),
    },
  });
  reportProgress(caseIndex);
}

for (const row of rows) {
  const hasPropertyCandidate = Object.hasOwn(
    row.baselineCandidates,
    "terser-properties",
  );
  if (hasPropertyCandidate === row.terserProperties) continue;
  const message = row.terserProperties
    ? `${row.name}: eligible Terser property candidate was not recorded`
    : `${row.name}: opted-out Terser property candidate was unexpectedly recorded`;
  failures.push(message);
  row.failures.push(message);
  row.passed = false;
}

rows.sort((left, right) => compareCodeUnits(left.name, right.name));
const passedCases = rows.filter((row) => row.passed).length;
const failedCases = rows.length - passedCases;
const selectedBy = only ?? (canonicalOnly ? "canonical" : "all");
const codecProvenance = canonicalCodecProvenance("comparison/cases report");
const versions = toolVersions(compilerProvenance);
const configProvenance = Object.fromEntries(
  lanes.map((lane) => [
    lane.name,
    {
      path: lane.config.slice(repo.length + 1),
      digest: sha256(readFileSync(lane.config)),
    },
  ]),
);
const deterministicResultsDigest = sha256(
  JSON.stringify({
    selectedBy,
    oracle: checkedInOracle,
    generatedCorpusDigest: catalogEvaluation.corpusDigest,
    completeCorpusDigest,
    configs: configProvenance,
    tools: {
      node: versions.node,
      lilscript: versions.lilscript.digest,
      terser: versions.terser,
      oxcViaRolldown: versions.oxcViaRolldown,
      esbuild: versions.esbuild,
      codec: codecProvenance.scorer.sha256,
    },
    rows: withoutDiagnosticTimings(rows),
  }),
);
const report = {
  schemaVersion: 6,
  catalogCases: catalogEntries.length,
  canonicalCases: canonicalEntries.length,
  // Keep full reports self-identifying. A JSON null is too easy to mistake for
  // missing provenance when a focused run can overwrite the same build file.
  selectedBy,
  cases: rows.length,
  passedCases,
  failedCases,
  failureEvents: failures.length,
  failureDetails: failures,
  coverage: {
    minimumUniqueGeneratedBehaviors: MIN_UNIQUE_GENERATED_BEHAVIORS,
    generated: catalogCoverage,
    canonical: canonicalCoverage,
    complete: fullCoverage,
    generatedBehaviorLabelAudit,
    selected: summarizeBehaviorCoverage(selected, {
      label: "selected micro corpus",
    }),
    compilerObjectiveRows: rows.length * lanes.length,
    baselineCandidateRows: rows.reduce(
      (total, row) => total + Object.keys(row.baselineCandidates).length,
      0,
    ),
    terserPropertyMangling: {
      eligibleCases: [...catalogEntries, ...canonicalEntries].filter(
        (entry) => entry.terserProperties,
      ).length,
      excludedCases: [...catalogEntries, ...canonicalEntries]
        .filter((entry) => !entry.terserProperties)
        .map((entry) => ({
          name: entry.name,
          reason: entry.terserPropertyReason,
        })),
      selectedEligibleCases: selected.filter((entry) => entry.terserProperties)
        .length,
      selectedCandidateRows: rows.filter((row) =>
        Object.hasOwn(row.baselineCandidates, "terser-properties"),
      ).length,
      selectedSemanticallyValidCandidates: rows.filter(
        (row) =>
          row.baselineCandidates["terser-properties"]?.semanticValid === true,
      ).length,
      selectedSemanticallyInvalidCandidates: rows.filter(
        (row) =>
          row.baselineCandidates["terser-properties"]?.semanticValid === false,
      ).length,
      missingEligibleCandidateRows: rows.filter(
        (row) =>
          row.terserProperties &&
          !Object.hasOwn(row.baselineCandidates, "terser-properties"),
      ).length,
      eligibilityIsFailClosed: true,
    },
  },
  expectations: {
    le: "LilScript must be no larger in each metric-specific compiler lane",
    lt: "LilScript must be strictly smaller in each metric-specific compiler lane",
  },
  lilscriptLaneContract: Object.fromEntries(
    lanes.map((lane) => [
      lane.name,
      {
        config: lane.config.slice(repo.length + 1),
        gateMetric: lane.metric,
        diagnosticMetrics: metrics.filter((metric) => metric !== lane.metric),
      },
    ]),
  ),
  codecs: codecProvenance,
  javascriptTarget,
  baselineOptions,
  provenance: {
    oracle: checkedInOracle,
    corpusDigest: catalogEvaluation.corpusDigest,
    generatedCorpusDigest: catalogEvaluation.corpusDigest,
    completeCorpusDigest,
    runnerDigest: sha256(readFileSync(fileURLToPath(import.meta.url))),
    coverageContractDigest: sha256(readFileSync(join(here, "coverage.mjs"))),
    deterministicResultsDigest,
    deterministicResultsExclude: [
      "rows[].baselineCandidates.*.durationMs",
      "rows[].lilscript.*.compileDurationMs",
    ],
    configs: configProvenance,
  },
  toolVersions: versions,
  runtime: {
    platform: process.platform,
    architecture: process.arch,
    versions: {
      node: process.versions.node,
      v8: process.versions.v8,
      zlib: process.versions.zlib,
      brotli: process.versions.brotli,
      modules: process.versions.modules,
    },
    nodeCodecMeasurements: "diagnostic-only; hard gates use lilscript-codec",
  },
  strictWins,
  nonLosses,
  baselineToolWins,
  rows,
};
writeFileSync(
  join(here, "summary.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

const header =
  "| Case | Expect | Lil raw-target | JS raw | Tool | Lil gzip-target | JS gzip | Tool | Lil Brotli-target | JS Brotli | Tool |\n" +
  "| --- | --- | ---: | ---: | --- | ---: | ---: | --- | ---: | ---: | --- |\n";
const table = rows
  .map((row) => {
    const raw = row.lilscript.raw?.size ?? "error";
    const gzip = row.lilscript.gzip9?.size ?? "error";
    const brotli = row.lilscript.brotli11?.size ?? "error";
    const jsRaw = row.javascript.raw;
    const jsGzip = row.javascript.gzip9;
    const jsBrotli = row.javascript.brotli11;
    return `| ${row.name} | ${row.expect} | ${raw} | ${jsRaw?.size ?? "error"} | ${jsRaw?.tool ?? "error"} | ${gzip} | ${jsGzip?.size ?? "error"} | ${jsGzip?.tool ?? "error"} | ${brotli} | ${jsBrotli?.size ?? "error"} | ${jsBrotli?.tool ?? "error"} |`;
  })
  .join("\n");
writeFileSync(
  join(here, "summary.md"),
  `# Web minifier micro suite\n\n` +
    `${rows.length} selected cases; ${passedCases} passed, ${failedCases} failed ` +
    `with ${failures.length} failure events. ` +
    `${catalogCoverage.uniqueBehaviorTemplates} generated behavior templates and ` +
    `${canonicalCoverage.caseInstances} independently reviewed canonical cases; ` +
    `parameter variants are reported separately. ` +
    `Strict objective wins — raw-target/raw ${strictWins.raw}, ` +
    `gzip-target/gzip ${strictWins.gzip9}, ` +
    `Brotli-target/Brotli ${strictWins.brotli11}.\n\n` +
    `Each metric uses a separately configured LilScript artifact and the smallest valid Terser, Oxc, or esbuild artifact for that metric.\n\n` +
    `${header}${table}\n`,
);

if (failures.length > 0) {
  console.error(failures.join("\n\n"));
  console.error(
    `\n${failedCases} failed cases (${failures.length} failure events) / ${selected.length} selected. ` +
      "See comparison/cases/summary.md",
  );
  process.exit(1);
}

console.log(
  `Web minifier suite passed: ${rows.length} cases; strict wins raw=${strictWins.raw}, gzip=${strictWins.gzip9}, brotli=${strictWins.brotli11}.`,
);
