import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { platform } from "node:os";
import { build as esbuild, transformSync as esbuildTransform } from "esbuild";
import { minify as terserMinify } from "terser";
import { build as viteBuild } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const codecScorer = join(
  repoRoot,
  `target/release/lilscript-codec${process.platform === "win32" ? ".exe" : ""}`,
);
const buildRoot = join(labRoot, "build");
const brotliObjectiveConfig = join(repoRoot, "lilscript.toml");
const closure = join(
  labRoot,
  "node_modules",
  ".bin",
  platform() === "win32"
    ? "google-closure-compiler.cmd"
    : "google-closure-compiler",
);

function metrics(buf) {
  return canonicalCodecSizes(buf);
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function run(program, args, cwd = labRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

function compileBrotliModule(source, output) {
  const portId = source.match(/[/\\]ports[/\\]([^/\\]+)[/\\]/)?.[1];
  const config = portId ? portConfig(portId) : brotliObjectiveConfig;
  run(compiler, [
    source,
    "--target",
    "js-module",
    "--config",
    config,
    "-o",
    output,
  ]);
}

function portConfig(id) {
  const specific = join(labRoot, "ports", id, "lilscript.toml");
  return existsSync(specific) ? specific : brotliObjectiveConfig;
}

function execute(path) {
  const result = spawnSync(process.execPath, [path], {
    cwd: labRoot,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`${path}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

async function buildViteLane(root, name, expected, external = []) {
  const outDir = join(buildRoot, name);
  await viteBuild({
    root,
    base: "./",
    configFile: false,
    logLevel: "silent",
    build: {
      outDir,
      emptyOutDir: true,
      minify: true,
      modulePreload: { polyfill: false },
      rollupOptions: external.length > 0 ? { external } : undefined,
    },
  });
  const jsAssets = readdirSync(join(outDir, "assets"))
    .filter((file) => file.endsWith(".js"))
    .sort();
  if (jsAssets.length !== 1) {
    throw new Error(
      `${name}: expected exactly one eager JavaScript artifact; found ${jsAssets.length}. ` +
        "Use a manifest-aware chunk lane before measuring split output.",
    );
  }
  const code = readFileSync(join(outDir, "assets", jsAssets[0]));
  const executable = join(buildRoot, `${name}-run.mjs`);
  writeFileSync(executable, code);
  const stdout = execute(executable);
  if (stdout !== expected) {
    throw new Error(`${name} contract failed:\n${stdout}\n!=\n${expected}`);
  }
  return code;
}

async function bundleLane(entry, name, expected, external = []) {
  const result = await esbuild({
    absWorkingDir: labRoot,
    bundle: true,
    entryPoints: [entry],
    format: "esm",
    platform: "browser",
    write: false,
    minify: false,
    external,
  });
  const code = result.outputFiles[0].text;
  const executable = join(buildRoot, `${name}-raw.mjs`);
  writeFileSync(executable, code);
  const stdout = execute(executable);
  if (stdout !== expected) {
    throw new Error(`${name} raw contract failed:\n${stdout}\n!=\n${expected}`);
  }
  return code;
}

async function terserLane(code, name, expected) {
  const result = await terserMinify(code, {
    module: true,
    compress: { passes: 3 },
    mangle: true,
  });
  const output = join(buildRoot, `${name}-terser.mjs`);
  writeFileSync(output, result.code);
  const stdout = execute(output);
  if (stdout !== expected) {
    throw new Error(
      `${name} Terser contract failed:\n${stdout}\n!=\n${expected}`,
    );
  }
  return result.code;
}

function closureLane(code, name, expected, options = {}) {
  const {
    compilationLevel = "ADVANCED",
    transpile = false,
    jscompOff = [],
    externProperties = [],
  } = options;
  const input = join(buildRoot, `${name}-closure-input.js`);
  const output = join(buildRoot, `${name}-closure.js`);
  const prepared = transpile
    ? esbuildTransform(code, {
        loader: "js",
        target: "es2020",
        format: "esm",
      }).code
    : code;
  writeFileSync(input, prepared);
  const args = [
    "--js",
    input,
    "--js_output_file",
    output,
    "--compilation_level",
    compilationLevel,
    "--language_in",
    "ECMASCRIPT_2021",
    "--language_out",
    "ECMASCRIPT_2021",
    "--warning_level",
    "QUIET",
    "--emit_use_strict=false",
    "--rewrite_polyfills=false",
  ];
  if (externProperties.length > 0) {
    const invalid = externProperties.find(
      (property) => !/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(property),
    );
    if (invalid) throw new Error(`invalid Closure extern property: ${invalid}`);
    const externs = join(buildRoot, `${name}-closure-externs.js`);
    writeFileSync(
      externs,
      `/** @externs */\nfunction PublishedLibraryApi(){}\n${[
        ...new Set(externProperties),
      ]
        .sort()
        .map((property) => `PublishedLibraryApi.prototype.${property};`)
        .join("\n")}\n`,
    );
    args.push("--externs", externs);
  }
  for (const flag of jscompOff) {
    args.push("--jscomp_off", flag);
  }
  run(closure, args);
  const stdout = execute(output);
  if (stdout !== expected) {
    throw new Error(
      `${name} Closure contract failed:\n${stdout}\n!=\n${expected}`,
    );
  }
  return readFileSync(output);
}

async function measureProject(spec) {
  const {
    id,
    project,
    eligibility,
    blockers,
    expected,
    viteName = `${id}-vite`,
    lilViteName = `${id}-lilscript-vite`,
    rawName = id,
    lilRawName = `${id}-lilscript`,
    jsRoot = join(labRoot, `apps/${id}/js`),
    lilRoot = join(labRoot, `apps/${id}/lil`),
    jsEntry = `apps/${id}/js/main.js`,
    lilEntry = `apps/${id}/lil/main.js`,
    closureOptions = {},
    verification = null,
    external = [],
    costModel = "brotli",
  } = spec;

  if (costModel !== "brotli") {
    throw new Error(
      `${id}: this runner emits ${relative(repoRoot, brotliObjectiveConfig)} ` +
        `Brotli-objective modules, so costModel must be "brotli"`,
    );
  }

  const vite = await buildViteLane(jsRoot, viteName, expected, external);
  const lilVite = await buildViteLane(lilRoot, lilViteName, expected);
  const rawJs = await bundleLane(jsEntry, rawName, expected, external);
  const lilRaw = await bundleLane(lilEntry, lilRawName, expected);
  const terser = await terserLane(rawJs, rawName, expected);
  const closureCode = closureLane(rawJs, rawName, expected, closureOptions);

  return {
    id,
    project,
    eligibility,
    blockers,
    verification,
    closureLevel: closureOptions.compilationLevel ?? "ADVANCED",
    costModel,
    objectiveContract: {
      artifact: "lilscriptVite",
      gateMetric: costModel,
      config: relative(repoRoot, brotliObjectiveConfig),
      diagnosticMetrics: ["raw", "gzip", "brotli"].filter(
        (metric) => metric !== costModel,
      ),
      diagnosticCrossMetricsMayLose: true,
    },
    codecs: canonicalCodecProvenance(),
    compiler: {
      path: relative(repoRoot, compiler),
      sha256: sha256File(compiler),
      configSha256: sha256File(brotliObjectiveConfig),
    },
    rawJs: metrics(rawJs),
    terser: metrics(terser),
    closure: metrics(closureCode),
    vite: metrics(vite),
    lilscript: metrics(lilRaw),
    lilscriptVite: metrics(lilVite),
    expected,
  };
}

function formatRow(table) {
  return `| ${table.project} | ${table.rawJs.raw} / gz ${table.rawJs.gzip} / br ${table.rawJs.brotli} | ${table.terser.raw} / gz ${table.terser.gzip} / br ${table.terser.brotli} | ${table.closureLevel}: ${table.closure.raw} / gz ${table.closure.gzip} / br ${table.closure.brotli} | ${table.vite.raw} / gz ${table.vite.gzip} / br ${table.vite.brotli} | ${table.lilscript.raw} / gz ${table.lilscript.gzip} / br ${table.lilscript.brotli} | ${table.lilscriptVite.raw} / gz ${table.lilscriptVite.gzip} / br ${table.lilscriptVite.brotli} | ${table.lilscriptVite.brotli} / ${table.vite.brotli} |`;
}

function formatSizeTriplet(size) {
  return `${size.raw} / gz ${size.gzip} / br ${size.brotli}`;
}

function normalizeExternalSize(size) {
  return {
    raw: size.raw,
    gzip: size.gzip9 ?? size.gzip,
    brotli: size.brotli11 ?? size.brotli,
  };
}

function solidLabReportPaths() {
  const paths = [];
  if (process.env.LILSCRIPT_SOLID_LAB) {
    paths.push(
      join(process.env.LILSCRIPT_SOLID_LAB, "artifacts/size-report.json"),
    );
  }
  paths.push(join(repoRoot, "labs/solid-client/artifacts/size-report.json"));
  paths.push(join(labRoot, "apps/solid/size-report.json"));
  return paths;
}

function loadSolidExternalTable() {
  for (const reportPath of solidLabReportPaths()) {
    if (!existsSync(reportPath)) {
      continue;
    }
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    const solidSize = report.sizes?.["solid-todolist"];
    const solidlilSize = report.sizes?.["solidlil-todolist"];
    if (!solidSize || !solidlilSize) {
      console.log(`note: ${reportPath} has no integrated LSX application lane`);
      continue;
    }
    const solid = normalizeExternalSize(solidSize);
    const solidlil = normalizeExternalSize(solidlilSize);
    const evidenceStatus = report.evidence?.status ?? "external-current";
    const archived = evidenceStatus === "archived-external-snapshot";
    const source = reportPath.endsWith("apps/solid/size-report.json")
      ? "archived apps/solid/size-report.json"
      : reportPath;
    console.log(`Solid / solidlil LSX sizes from ${source}`);
    return {
      id: "solid-js",
      project: "Solid / solidlil LSX todolist",
      eligibility: "partial-external",
      blockers: [
        archived
          ? "Archived sibling-worktree application snapshot; LSX parsing/lowering is integrated, but the todolist and its gates are not yet reproducible from the monorepo lab."
          : "Measured by the integrated Solid client lab, outside this runner.",
        "Same todo interaction contract; Vite/oxc-minified full app JS. Not a complete Solid replacement.",
      ],
      external: true,
      evidenceStatus,
      codecEvidence: archived ? "legacy-unknown" : "external-unattested",
      codecs: null,
      compiler: null,
      costModel: "brotli",
      source: reportPath.startsWith(repoRoot)
        ? relative(repoRoot, reportPath)
        : reportPath,
      rawJs: { raw: "—", gzip: "—", brotli: "—" },
      terser: { raw: "—", gzip: "—", brotli: "—" },
      closure: { raw: "—", gzip: "—", brotli: "—" },
      vite: solid,
      lilscript: { raw: "—", gzip: "—", brotli: "—" },
      lilscriptVite: solidlil,
      comparisons: report.comparisons?.todolistLilx ?? null,
      expected: "external:solid-lab-todolist-contract",
    };
  }
  console.log(
    "note: skipping Solid / solidlil row — neither the integrated lab nor the archived apps/solid snapshot has an LSX application report",
  );
  return null;
}

function formatExternalRow(table) {
  return `| ${table.project} | — | — | — | ${formatSizeTriplet(table.vite)} | — | ${formatSizeTriplet(table.lilscriptVite)} | ${table.lilscriptVite.brotli} / ${table.vite.brotli} |`;
}

mkdirSync(buildRoot, { recursive: true });

if (
  !/^cost_model\s*=\s*["']brotli["']\s*$/m.test(
    readFileSync(brotliObjectiveConfig, "utf8"),
  )
) {
  throw new Error(
    `${brotliObjectiveConfig} must declare cost_model = "brotli"`,
  );
}

if (!existsSync(compiler) || !existsSync(codecScorer)) {
  run(
    process.env.CARGO ?? "cargo",
    ["build", "--release", "--bin", "lilscript", "--bin", "lilscript-codec"],
    repoRoot,
  );
}
requireCanonicalCodecRuntime("popular-library publication gate");

const nanoidExpected = readFileSync(
  join(labRoot, "apps/nanoid/expected.txt"),
  "utf8",
).trim();
const lilModule = join(buildRoot, "nanoid-lilscript-module.js");
compileBrotliModule(join(labRoot, "ports/nanoid/index.lil"), lilModule);
const nanoidUpstreamStdout = run(process.execPath, [
  join(labRoot, "verify-nanoid.mjs"),
]);
if (nanoidUpstreamStdout !== "nanoid-upstream:2") {
  throw new Error(
    `nanoid upstream assertions failed:\n${nanoidUpstreamStdout}`,
  );
}

const nanoidTable = await measureProject({
  id: "nanoid",
  project: "Nano ID",
  eligibility: "exact-browser-entrypoint",
  blockers: [
    "This is the published browser entrypoint, not Nano ID's distinct pooled Node entrypoint.",
  ],
  verification: {
    differential: nanoidUpstreamStdout,
    selectedEntrypoint: "nanoid/index.browser.js",
  },
  expected: nanoidExpected,
});

const lilOut = join(buildRoot, "nanoid-lilscript-contract.js");
const lilMerged = join(buildRoot, "nanoid-lilscript-run.js");
run(compiler, [
  join(labRoot, "apps/nanoid/lil/main.lil"),
  "--target",
  "js",
  "-o",
  lilOut,
]);
writeFileSync(lilMerged, `${readFileSync(lilOut, "utf8")}\n`);
const lilStdout = execute(lilMerged);
if (lilStdout !== nanoidExpected) {
  throw new Error(
    `lilscript contract failed:\n${lilStdout}\n!=\n${nanoidExpected}`,
  );
}
const nanoidNativeC = join(buildRoot, "nanoid-lilscript-native.c");
const nanoidNativeExecutable = join(buildRoot, "nanoid-lilscript-native");
run(compiler, [
  join(labRoot, "apps/nanoid/lil/main-native.lil"),
  "--target",
  "c",
  "-o",
  nanoidNativeC,
]);
run("clang", [
  "-std=c11",
  "-O3",
  nanoidNativeC,
  join(labRoot, "apps/nanoid/lil/host.c"),
  "-lm",
  "-o",
  nanoidNativeExecutable,
]);
const nanoidNativeStdout = run(nanoidNativeExecutable, []);
if (nanoidNativeStdout !== nanoidExpected) {
  throw new Error(
    `nanoid native contract failed:\n${nanoidNativeStdout}\n!=\n${nanoidExpected}`,
  );
}

const mittExpected = readFileSync(
  join(labRoot, "apps/mitt/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/mitt/index.lil"),
  join(buildRoot, "mitt-lilscript.js"),
);
const mittVerifierStdout = run(process.execPath, [
  join(labRoot, "verify-mitt.mjs"),
]);
if (mittVerifierStdout !== "mitt-upstream:2:10") {
  throw new Error(
    `mitt differential assertions failed:\n${mittVerifierStdout}`,
  );
}
const mittTable = await measureProject({
  id: "mitt",
  project: "mitt",
  eligibility: "exact-root-entrypoint",
  blockers: [],
  verification: {
    differential: mittVerifierStdout,
    selectedEntrypoint: "mitt",
  },
  expected: mittExpected,
  closureOptions: {
    // These are observable fields of mitt's returned public emitter. Closure
    // ADVANCED may optimize through them, but it may not rename the API that
    // LilScript and the published package both preserve.
    externProperties: ["all", "emit", "off", "on"],
  },
});

const clsxExpected = readFileSync(
  join(labRoot, "apps/clsx/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/clsx/index.lil"),
  join(buildRoot, "clsx-lilscript.js"),
);
const clsxVerifierStdout = run(process.execPath, [
  join(labRoot, "verify-clsx.mjs"),
]);
if (clsxVerifierStdout !== "clsx-differential:10000") {
  throw new Error(
    `clsx differential assertions failed:\n${clsxVerifierStdout}`,
  );
}
const clsxTable = await measureProject({
  id: "clsx",
  project: "clsx",
  eligibility: "exact-root-entrypoint",
  blockers: [],
  verification: {
    differential: clsxVerifierStdout,
    selectedEntrypoint: "clsx",
  },
  expected: clsxExpected,
});

const immerExpected = readFileSync(
  join(labRoot, "apps/immer/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/immer/index.lil"),
  join(buildRoot, "immer-lilscript.js"),
);
const immerTable = await measureProject({
  id: "immer",
  project: "immer",
  eligibility: "candidate",
  blockers: [
    "LilScript has no Proxies; the measured JS facade restores produce/current/original mutation syntax over an explicit ImmValue COW draft tree.",
    "Patch generation lives in LilScript; applyPatches is restored in the measured JS facade over plain clones. Map/Set/Date/class/immerable drafts, freeze, and manual createDraft/finishDraft remain out of this subset.",
  ],
  expected: immerExpected,
});

const rtkExpected = readFileSync(
  join(labRoot, "apps/redux-toolkit/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/redux-toolkit/index.lil"),
  join(buildRoot, "redux-toolkit-lilscript.js"),
);
const rtkTable = await measureProject({
  id: "redux-toolkit",
  project: "Redux Toolkit core subset",
  eligibility: "candidate",
  blockers: [
    "createSlice draft reducers reuse ports/immer COW; the measured JS facade restores RTK-shaped createSlice/configureStore and Proxy mutation syntax.",
    "No middleware, thunks, DevTools, RTK Query, createAsyncThunk, listener middleware, entity adapters, or reselect integration.",
    "npm Closure Advanced breaks RTK's dynamic action/reducer property model; this row uses Closure SIMPLE after an es2020 transpile.",
  ],
  expected: rtkExpected,
  closureOptions: {
    compilationLevel: "SIMPLE",
    transpile: true,
    jscompOff: ["checkVars"],
  },
});

const zodExpected = readFileSync(
  join(labRoot, "apps/zod/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/zod/index.lil"),
  join(buildRoot, "zod-lilscript.js"),
);
const zodTable = await measureProject({
  id: "zod",
  project: "Zod 3 core subset",
  eligibility: "candidate",
  blockers: [
    "Inputs use an explicit JsonValue kind tree; the JS facade restores the chainable z API.",
    "Email is a portable character-class approximation of Zod 3's regex because LilScript has no RegExp intrinsic.",
    ".transform() is applied in the JS facade after LilScript parse; refinements, coerce, effects, pipe, and Zod 4 remain unported.",
    "npm Closure Advanced mangles Zod issue codes and object shape keys; this row uses Closure SIMPLE.",
  ],
  expected: zodExpected,
  closureOptions: {
    compilationLevel: "SIMPLE",
    transpile: true,
    jscompOff: ["checkVars"],
  },
});

const acornExpected = readFileSync(
  join(labRoot, "apps/acorn/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/acorn/index.lil"),
  join(buildRoot, "acorn-lilscript.js"),
);
const acornTable = await measureProject({
  id: "acorn",
  project: "Acorn 8 parse subset",
  eligibility: "candidate",
  blockers: [
    "Fixed 25-program ecmaVersion 2020 script subset; fingerprints AST shape without loc/range/start/end/raw.",
    "Yield/generators, import/export modules, tokenizer/TokenType API, plugins, locations, and remaining operators/grammar remain unported.",
  ],
  expected: acornExpected,
});

const preactExpected = readFileSync(
  join(labRoot, "apps/preact/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "apps/preact/lil/main.lil"),
  join(buildRoot, "preact-lilscript.js"),
);
const preactTable = await measureProject({
  id: "preact",
  project: "Preact 10 core subset",
  eligibility: "candidate",
  blockers: [
    "Port covers h, Fragments, function/class components, useState/useRef/useMemo/useCallback/useLayoutEffect, list keys, and shared-stub DOM mount — not Preact's keyed reconciler, context, or Suspense.",
    "Both lanes mount into apps/preact/shared DOM stub + canon serializer; LilScript remounts on update (effects re-flushed) rather than in-place keyed diff.",
    "LilScript has no class extends, so class components use Component + renderFn / hClass versus npm extends Component.",
    "npm Closure Advanced mangles the class prop string; this row uses Closure SIMPLE.",
  ],
  expected: preactExpected,
  closureOptions: {
    compilationLevel: "SIMPLE",
  },
});

const glExpected = readFileSync(
  join(labRoot, "apps/gl-matrix/expected.txt"),
  "utf8",
).trim();
const glNativeExpected = readFileSync(
  join(labRoot, "apps/gl-matrix/native-expected.txt"),
  "utf8",
).trim();
const glExpectedSummary = glExpected.split("\n").join("`, `");
const glMatrixModules = [
  "vec2",
  "vec3",
  "vec4",
  "mat2",
  "mat2d",
  "mat3",
  "mat4",
  "quat",
  "quat2",
];
const glMatrixPublished = await import("gl-matrix/esm/index.js");
const glMatrixExternProperties = [
  ...new Set([
    ...Object.keys(glMatrixPublished),
    ...Object.values(glMatrixPublished).flatMap((module) =>
      module && typeof module === "object" ? Object.keys(module) : [],
    ),
  ]),
];
const glMatrixPmRoot = join(buildRoot, "gl-matrix-pm");
mkdirSync(glMatrixPmRoot, { recursive: true });
compileBrotliModule(
  join(labRoot, "ports/gl-matrix/entry.lil"),
  join(glMatrixPmRoot, "entry.js"),
);
writeFileSync(
  join(glMatrixPmRoot, "state.js"),
  `export let ARRAY_TYPE=typeof Float32Array!=="undefined"?Float32Array:Array;
export function setMatrixArrayType(type){ARRAY_TYPE=type}
export const createMatrixArray=size=>new ARRAY_TYPE(size);
`,
);
writeFileSync(
  join(glMatrixPmRoot, "common.js"),
  `export {ARRAY_TYPE,setMatrixArrayType} from "./state.js";
export var EPSILON=.000001,RANDOM=Math.random;
const degree=Math.PI/180;
export function toRadian(value){return value*degree}
export function equals(a,b){return Math.abs(a-b)<=EPSILON*Math.max(1,Math.abs(a),Math.abs(b))}
if(!Math.hypot)Math.hypot=function(){let sum=0,index=arguments.length;while(index--)sum+=arguments[index]*arguments[index];return Math.sqrt(sum)};
`,
);
for (const file of readdirSync(glMatrixPmRoot)) {
  if (!file.endsWith(".js") || file === "state.js" || file === "common.js")
    continue;
  const path = join(glMatrixPmRoot, file);
  const source = readFileSync(path, "utf8");
  const imports = [];
  if (source.includes("glMatrixCreateArray(")) {
    imports.push(
      `import {createMatrixArray as glMatrixCreateArray} from "./state.js";`,
    );
  }
  if (source.includes("rand(")) {
    imports.push(`import {RANDOM as rand} from "./common.js";`);
  }
  if (imports.length > 0)
    writeFileSync(path, `${imports.join("\n")}\n${source}`);
}
const glMatrixEntryLil = readFileSync(
  join(labRoot, "ports/gl-matrix/entry.lil"),
  "utf8",
);
const glMatrixApiParts = [
  `import * as glMatrix from "./common.js";`,
  `export {glMatrix};`,
];
for (const moduleName of glMatrixModules) {
  const importLine = glMatrixEntryLil
    .split("\n")
    .find((line) => line.includes(`from "./${moduleName}"`));
  if (!importLine) {
    throw new Error(`gl-matrix entry.lil missing import for ${moduleName}`);
  }
  const exportNames = [
    ...importLine.matchAll(new RegExp(`as ${moduleName}_([A-Za-z0-9_]+)`, "g")),
  ].map((match) => match[1]);
  if (exportNames.length === 0) {
    throw new Error(`gl-matrix entry.lil has no exports for ${moduleName}`);
  }
  writeFileSync(
    join(glMatrixPmRoot, `${moduleName}.js`),
    `export {\n${exportNames
      .map((name) => `  ${moduleName}_${name} as ${name}`)
      .join(",\n")}\n} from "./entry.js";\n`,
  );
  glMatrixApiParts.push(
    `import * as ${moduleName} from "./${moduleName}.js";\nexport {${moduleName}};`,
  );
}
writeFileSync(
  join(glMatrixPmRoot, "api.js"),
  `${glMatrixApiParts.join("\n")}\n`,
);

const glMatrixVerifierStdout = run(process.execPath, [
  join(labRoot, "verify-gl-matrix.mjs"),
]);
if (!glMatrixVerifierStdout.startsWith("gl-matrix-upstream:10:")) {
  throw new Error(
    `gl-matrix differential assertions failed:\n${glMatrixVerifierStdout}`,
  );
}

const glMatrixTable = await measureProject({
  id: "gl-matrix",
  project: "gl-matrix",
  eligibility: "exact-root-entrypoint",
  blockers: [],
  verification: {
    differential: glMatrixVerifierStdout,
    selectedEntrypoint: "gl-matrix",
  },
  expected: glExpected,
  viteName: "gl-matrix-vite",
  lilViteName: "gl-matrix-lilscript-vite",
  rawName: "gl-matrix",
  lilRawName: "gl-matrix-lilscript",
  closureOptions: {
    compilationLevel: "ADVANCED",
    // ADVANCED is allowed to optimize through the library, but the selected
    // root entrypoint's enumerable API is external. Supplying those published
    // names is the Closure equivalent of LilScript's properties=false policy.
    externProperties: glMatrixExternProperties,
  },
});
glMatrixTable.nativeExpected = glNativeExpected;

const motionExpected = readFileSync(
  join(labRoot, "apps/motion/expected.txt"),
  "utf8",
).trim();
compileBrotliModule(
  join(labRoot, "ports/motion/entry.lil"),
  join(buildRoot, "motion-lilscript.js"),
);
const motionVerifierStdout = run(process.execPath, [
  join(labRoot, "verify-motion.mjs"),
]);
if (!motionVerifierStdout.startsWith("motion-upstream:2:312:")) {
  throw new Error(
    `motion differential assertions failed:\n${motionVerifierStdout}`,
  );
}
const motionTable = await measureProject({
  id: "motion",
  project: "motion (mix/wrap/stagger/spring)",
  eligibility: "exact-selected-surface",
  blockers: [
    "Measured selected Motion 13 DOM surface used by the app: mix, wrap, stagger, spring (same algorithms as npm motion@13). Full DOM package completeness is tracked separately; React entrypoints are out of scope.",
  ],
  verification: {
    differential: motionVerifierStdout,
    selectedEntrypoint: "motion:mix+wrap+stagger+spring",
  },
  expected: motionExpected,
  closureOptions: {
    externProperties: ["next", "value"],
  },
});

const glNativeC = join(buildRoot, "gl-matrix-native.c");
const glNativeExecutable = join(buildRoot, "gl-matrix-native");
run(compiler, [
  join(labRoot, "apps/gl-matrix/lil/main.lil"),
  "--target",
  "c",
  "-o",
  glNativeC,
]);
run("clang", [
  "-std=c11",
  "-O3",
  join(labRoot, "apps/gl-matrix/lil/host.c"),
  "-lm",
  "-o",
  glNativeExecutable,
]);
const glNativeStdout = run(glNativeExecutable, []);
if (glNativeStdout !== glNativeExpected) {
  throw new Error(
    `gl-matrix native contract failed:\n${glNativeStdout}\n!=\n${glNativeExpected}`,
  );
}

const solidTable = loadSolidExternalTable();

const tables = [
  nanoidTable,
  mittTable,
  clsxTable,
  immerTable,
  rtkTable,
  zodTable,
  acornTable,
  preactTable,
  ...(solidTable ? [solidTable] : []),
  glMatrixTable,
  motionTable,
];

const solidProse = solidTable
  ? solidTable.evidenceStatus === "archived-external-snapshot"
    ? `
Solid / solidlil is an archived sibling-worktree snapshot: Solid JSX todolist
vs solidlil LSX (\`.lilx\` → LilScript reactive + LilScript DOM), same todo
contract, Vite/oxc-minified full app JS. Brotli ${solidTable.lilscriptVite.brotli} /
${solidTable.vite.brotli} (solidlil / Solid; ${
        solidTable.comparisons
          ? `${solidTable.comparisons.brotliPct.toFixed(1)}%`
          : "see archived report"
      }). The current integrated lab does not contain the LSX pipeline, so this row
is historical evidence rather than a reproducible single-repository gate.
`
    : `
Solid / solidlil is a partial external row from \`lilscript-solid-lab\`: Solid JSX
todolist vs solidlil LSX (\`.lilx\` → LilScript reactive + LilScript DOM), same todo
contract, Vite/oxc-minified full app JS. Brotli ${solidTable.lilscriptVite.brotli} /
${solidTable.vite.brotli} (solidlil / Solid; ${
        solidTable.comparisons
          ? `${solidTable.comparisons.brotliPct.toFixed(1)}%`
          : "see lab report"
      }). Raw/Terser/Closure columns are not measured in that lab lane.
`
  : `
Solid / solidlil was skipped because neither the integrated lab nor the archived
\`apps/solid/size-report.json\` snapshot contains an LSX application report.
`;

const publicTables = [
  nanoidTable,
  mittTable,
  clsxTable,
  glMatrixTable,
  motionTable,
];
const md = `# Exact-entrypoint popular library sizes

Only complete selected entrypoints appear in this comparison. Incomplete research
ports remain implementation backlog and their sizes are deliberately excluded.

Nano ID covers every export of \`index.browser.js\`, including defaults, coercions,
deterministic custom generators, randomness/distribution, and the 2147483648 callback
step. mitt covers its complete default-export surface and observable runtime function
shape. clsx preserves its default/named identity and recursive raw-JavaScript-value
algorithm without a conversion facade. gl-matrix covers the complete ESM root namespace, every module export and alias,
live \`ARRAY_TYPE\`, and \`setMatrixArrayType\` allocation behavior. motion measures the selected Motion 13
\`mix\`/\`wrap\`/\`stagger\`/\`spring\` surface used by the app (same equations as npm \`motion@13\`); full DOM
package completeness remains the compatibility backlog (React entrypoints are out of scope).

Each row uses the same app contract and Vite 8 settings. Adapter bytes are included.
Publication additionally requires differential behavior and no selected-codec
size regression against either npm/Vite 8 or public-API-preserving Closure ADVANCED,
and no material throughput or retained-memory regression. Gzip and Brotli remain
visible, along with raw, as diagnostics because a build tuned for one may trade
bytes in the others. The current publication objective is Brotli, so only the
Brotli cell of the Brotli-selected LilScript artifact is size-gated.
Closure ADVANCED receives generated externs for observable published properties, so
it may optimize through the app but may not rename the API being compared.

${solidProse}
The Solid/solidlil result above is an application benchmark, not a claim that the
complete Solid package surface has been reimplemented.

| Project | Raw JS | Terser | Closure (actual level) | npm Vite 8 | LilScript pre-Vite (diagnostic triplet) | LilScript Vite (Brotli objective) | Brotli (Lil / npm) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
${publicTables.map(formatRow).join("\n")}
`;

writeFileSync(join(labRoot, "RESULTS.md"), md);
writeFileSync(
  join(labRoot, "build/results.json"),
  `${JSON.stringify(tables, null, 2)}\n`,
);
console.log(md);
