import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { arch, platform } from "node:os";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
  requireCanonicalCodecRuntime,
} from "../../benchmarks/codec-contract.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const popular = join(repo, "benchmarks/popular");
const casesRoot = join(here, "cases");
const buildRoot = join(here, "build");
const hostPath = join(here, "host.cjs");
const closureCliPath = join(
  popular,
  "node_modules/google-closure-compiler/cli.js",
);
const summaryJsonPath = join(here, "summary.json");
const summaryMarkdownPath = join(here, "summary.md");
const require = createRequire(join(popular, "package.json"));
const metrics = ["raw", "gzip9", "brotli11"];
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
const lanes = [
  { name: "raw", metric: "raw", config: join(here, "configs/raw.toml") },
  { name: "gzip", metric: "gzip9", config: join(here, "configs/gzip.toml") },
  {
    name: "brotli",
    metric: "brotli11",
    config: join(here, "configs/brotli.toml"),
  },
];
const closureExterns =
  "/** @externs */\n" +
  "/** @param {number} index @return {number} */ function algorithmInt(index) {}\n" +
  "/** @param {number} index @return {string} */ function algorithmString(index) {}\n" +
  "/** @return {number} */ function algorithmCount() {}\n";

const baselineOptions = {
  target: "es2022",
  boundary: "runtime-host-script",
  terser: {
    ecma: 2022,
    module: false,
    compress: { ecma: 2022, passes: 3, drop_console: false, toplevel: true },
    mangle: { toplevel: true },
    format: { ecma: 2022, comments: false },
  },
  terserProperties: {
    ecma: 2022,
    module: false,
    compress: { ecma: 2022, passes: 3, drop_console: false, toplevel: true },
    mangle: {
      toplevel: true,
      properties: {
        regex: { source: "^_", flags: "u" },
        builtins: false,
        keep_quoted: "strict",
      },
    },
    format: { ecma: 2022, comments: false },
  },
  oxc: {
    module: false,
    compress: { target: "es2022" },
    mangle: { toplevel: true },
    codegen: { target: "es2022", legalComments: "none" },
  },
  closureAdvanced: {
    platformPriority: ["native", "java"],
    compilationLevel: "ADVANCED",
    languageIn: "ECMASCRIPT_2021",
    languageOut: "ECMASCRIPT_2021",
    es2022Compatibility: "ECMASCRIPT_2021 is a strict ES2022 subset",
    env: "BROWSER",
    warningLevel: "QUIET",
    rewritePolyfills: false,
    assumeFunctionWrapper: true,
  },
  closureAdvancedModuleGraph: {
    platformPriority: ["native", "java"],
    compilationLevel: "ADVANCED",
    dependencyMode: "PRUNE",
    entryPoint: "main.js",
    languageIn: "ECMASCRIPT_2021",
    languageOut: "ECMASCRIPT_2021",
    es2022Compatibility: "ECMASCRIPT_2021 is a strict ES2022 subset",
    env: "BROWSER",
    warningLevel: "QUIET",
    rewritePolyfills: false,
    assumeFunctionWrapper: true,
  },
  esbuildScript: {
    target: "es2022",
    minify: true,
    legalComments: "none",
  },
  esbuildIife: {
    target: "es2022",
    format: "iife",
    minify: true,
    legalComments: "none",
  },
  esbuildBundleIife: {
    bundle: true,
    platform: "neutral",
    format: "iife",
    target: "es2022",
    minify: true,
    treeShaking: true,
    legalComments: "none",
  },
  viteOxcBundleIife: {
    configFile: false,
    logLevel: "silent",
    publicDir: false,
    target: "es2022",
    minify: "oxc",
    library: { formats: ["iife"], name: "AlgorithmCase", fileName: "bundle" },
    emptyOutDir: true,
    copyPublicDir: false,
    sourcemap: false,
  },
  viteTerserBundleIife: {
    configFile: false,
    logLevel: "silent",
    publicDir: false,
    target: "es2022",
    minify: "terser",
    library: { formats: ["iife"], name: "AlgorithmCase", fileName: "bundle" },
    emptyOutDir: true,
    copyPublicDir: false,
    sourcemap: false,
    terserOptions: {
      ecma: 2022,
      module: false,
      compress: { ecma: 2022, passes: 3, drop_console: false, toplevel: true },
      mangle: { toplevel: true },
      format: { ecma: 2022, comments: false },
    },
  },
  moduleBundling: {
    bundle: true,
    platform: "neutral",
    format: "iife",
    target: "es2022",
    minify: false,
    treeShaking: true,
    legalComments: "none",
  },
};

const argv = process.argv.slice(2);
const onlyIndex = argv.indexOf("--only");
const only = onlyIndex === -1 ? null : argv[onlyIndex + 1];
for (const [index, argument] of argv.entries()) {
  if (index === onlyIndex + 1) continue;
  if (argument !== "--only") throw new Error(`unknown argument: ${argument}`);
}
if (onlyIndex !== -1 && !only)
  throw new Error("--only requires a non-empty substring");

const [nodeMajor, nodeMinor] = process.versions.node.split(".").map(Number);
if (
  nodeMajor < 20 ||
  (nodeMajor === 20 && nodeMinor < 19) ||
  nodeMajor === 21 ||
  (nodeMajor === 22 && nodeMinor < 12)
) {
  throw new Error(
    `Node ${process.versions.node} is unsupported; use Node 20.19+ or 22.12+ ` +
      `(the repository pins Node 24 in .nvmrc)`,
  );
}

rmSync(buildRoot, { recursive: true, force: true });
rmSync(summaryJsonPath, { force: true });
rmSync(summaryMarkdownPath, { force: true });

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

function sizeBand(bytes) {
  if (!Number.isFinite(bytes)) return null;
  if (bytes < 100) return "under-100B";
  if (bytes < 400) return "100-399B";
  if (bytes < 1000) return "400-999B";
  if (bytes <= 10 * 1024) return "1-10KiB";
  return "over-10KiB";
}

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(path) : [path];
  });
}

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? here,
    encoding: "utf8",
    env: options.env ?? process.env,
    input: options.input,
    maxBuffer: 32 * 1024 * 1024,
    timeout: options.timeout ?? 10 * 60 * 1000,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")} failed` +
        `${result.signal ? ` (${result.signal})` : ""}\n` +
        `${result.error?.message ?? ""}\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout;
}

function execute(source, vector) {
  const result = spawnSync(
    process.execPath,
    ["--require", hostPath, "--input-type=commonjs"],
    {
      cwd: here,
      input: source,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      timeout: 10_000,
      env: {
        ...process.env,
        LILSCRIPT_ALGORITHM_TRACE: "1",
        LILSCRIPT_ALGORITHM_VECTOR: JSON.stringify({
          ints: vector.ints,
          strings: vector.strings,
        }),
      },
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `algorithm execution failed${result.signal ? ` (${result.signal})` : ""}\n` +
        `${result.error?.message ?? ""}\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  const tracePrefix = "LILSCRIPT_ALGORITHM_TRACE=";
  const stderrLines = (result.stderr ?? "").split("\n").filter(Boolean);
  const traceLines = stderrLines.filter((line) => line.startsWith(tracePrefix));
  const unexpectedStderr = stderrLines.filter(
    (line) => !line.startsWith(tracePrefix),
  );
  if (traceLines.length !== 1 || unexpectedStderr.length !== 0) {
    throw new Error(
      `algorithm execution emitted an invalid host trace\n${result.stderr ?? ""}`,
    );
  }
  let hostAccessTrace;
  try {
    hostAccessTrace = JSON.parse(traceLines[0].slice(tracePrefix.length));
  } catch (error) {
    throw new Error(
      `algorithm execution emitted malformed host trace: ${error.message}`,
    );
  }
  return { stdout: result.stdout, hostAccessTrace };
}

function discoverCases() {
  const entries = readdirSync(casesRoot)
    .map((name) => join(casesRoot, name))
    .filter((path) => statSync(path).isDirectory())
    .map((directory) => {
      const id = basename(directory);
      const metadataPath = join(directory, "case.json");
      const lilPath = join(directory, "main.lil");
      const jsPath = join(directory, "main.js");
      for (const path of [metadataPath, lilPath, jsPath]) {
        if (!existsSync(path))
          throw new Error(`${id}: missing ${relative(repo, path)}`);
      }
      const lilSources = Object.fromEntries(
        filesUnder(directory)
          .filter((path) => path.endsWith(".lil"))
          .sort()
          .map((path) => [
            relative(directory, path).replaceAll("\\", "/"),
            readFileSync(path, "utf8"),
          ]),
      );
      const javascriptSources = Object.fromEntries(
        filesUnder(directory)
          .filter((path) => path.endsWith(".js"))
          .sort()
          .map((path) => [
            relative(directory, path).replaceAll("\\", "/"),
            readFileSync(path, "utf8"),
          ]),
      );
      return {
        id,
        directory,
        metadataPath,
        lilPath,
        jsPath,
        metadataSource: readFileSync(metadataPath, "utf8"),
        lilSources,
        javascriptSources,
        lil: lilSources["main.lil"],
        js: javascriptSources["main.js"],
      };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
  if (entries.length < 11)
    throw new Error(
      `algorithm corpus requires at least 11 cases, found ${entries.length}`,
    );
  return entries;
}

function analyzeJavaScriptGraph(entry, parse) {
  const functions = new Map();
  const programs = new Map();
  const visit = (node, callback) => {
    if (!node || typeof node !== "object") return;
    callback(node);
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) {
        for (const child of value) visit(child, callback);
      } else if (
        value &&
        typeof value === "object" &&
        typeof value.type === "string"
      ) {
        visit(value, callback);
      }
    }
  };
  for (const [module, source] of Object.entries(entry.javascriptSources)) {
    const ast = parse(source, {
      ecmaVersion: 2022,
      sourceType: "module",
      allowHashBang: true,
    });
    programs.set(module, ast);
    visit(ast, (node) => {
      if (node.type !== "FunctionDeclaration" || !node.id) return;
      if (functions.has(node.id.name)) {
        throw new Error(
          `${entry.id}: duplicate JavaScript function name ${node.id.name}`,
        );
      }
      functions.set(node.id.name, { module, node });
    });
  }
  const importsByModule = Object.fromEntries(
    [...programs].map(([module, ast]) => [
      module.replace(/\.js$/u, ""),
      ast.body
        .filter((node) => node.type === "ImportDeclaration")
        .map((node) =>
          node.source.value.replace(/^\.\//u, "").replace(/\.js$/u, ""),
        )
        .sort(),
    ]),
  );
  const edges = new Map(
    [...functions].map(([name, declaration]) => {
      const called = new Set();
      visit(declaration.node.body, (node) => {
        if (
          node.type === "CallExpression" &&
          node.callee.type === "Identifier" &&
          functions.has(node.callee.name)
        ) {
          called.add(node.callee.name);
        }
      });
      return [name, called];
    }),
  );
  const roots = new Set();
  const visitTopLevel = (node) => {
    if (!node || typeof node !== "object") return;
    if (
      [
        "FunctionDeclaration",
        "FunctionExpression",
        "ArrowFunctionExpression",
      ].includes(node.type)
    ) {
      return;
    }
    if (
      node.type === "CallExpression" &&
      node.callee.type === "Identifier" &&
      functions.has(node.callee.name)
    ) {
      roots.add(node.callee.name);
    }
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) {
        for (const child of value) visitTopLevel(child);
      } else if (
        value &&
        typeof value === "object" &&
        typeof value.type === "string"
      ) {
        visitTopLevel(value);
      }
    }
  };
  const entryProgram = programs.get("main.js");
  if (!entryProgram)
    throw new Error(`${entry.id}: JavaScript graph is missing main.js`);
  for (const statement of entryProgram.body) visitTopLevel(statement);
  if (roots.size === 0)
    throw new Error(
      `${entry.id}: JavaScript graph has no top-level entry call`,
    );
  const reachable = new Set();
  const markReachable = (name) => {
    if (reachable.has(name)) return;
    reachable.add(name);
    for (const called of edges.get(name)) markReachable(called);
  };
  for (const root of roots) markReachable(root);
  const memo = new Map();
  const visiting = new Set();
  const depth = (name) => {
    if (memo.has(name)) return memo.get(name);
    if (visiting.has(name))
      throw new Error(`${entry.id}: recursive call cycle includes ${name}`);
    visiting.add(name);
    const value = 1 + Math.max(0, ...[...edges.get(name)].map(depth));
    visiting.delete(name);
    memo.set(name, value);
    return value;
  };
  return {
    functions: functions.size,
    functionNamesByModule: Object.fromEntries(
      [...programs.keys()].sort().map((module) => [
        module.replace(/\.js$/u, ""),
        [...functions]
          .filter(([, declaration]) => declaration.module === module)
          .map(([name]) => name)
          .sort(),
      ]),
    ),
    importsByModule,
    callDepth: Math.max(...[...reachable].map(depth)),
    reachableFunctions: [...reachable].sort(),
    reachableModules: [
      ...new Set([...reachable].map((name) => functions.get(name).module)),
    ].sort(),
  };
}

function validateCase(entry, parse) {
  let metadata;
  try {
    metadata = JSON.parse(entry.metadataSource);
  } catch (error) {
    throw new Error(`${entry.id}: invalid case.json: ${error.message}`);
  }
  if (metadata.schemaVersion !== 1)
    throw new Error(`${entry.id}: schemaVersion must be 1`);
  if (metadata.id !== entry.id)
    throw new Error(`${entry.id}: metadata id must match folder`);
  if (typeof metadata.title !== "string" || !metadata.title)
    throw new Error(`${entry.id}: title is required`);
  if (typeof metadata.hypothesis !== "string" || !metadata.hypothesis.trim()) {
    throw new Error(`${entry.id}: hypothesis is required`);
  }
  const hypothesisSentences =
    metadata.hypothesis.match(/[^.!?]+(?:[.!?]+|$)/g) ?? [];
  if (hypothesisSentences.length < 1 || hypothesisSentences.length > 3) {
    throw new Error(`${entry.id}: hypothesis must contain 1-3 sentences`);
  }
  if (
    !["small-structural", "medium-structural", "large-structural"].includes(
      metadata.tier,
    )
  ) {
    throw new Error(`${entry.id}: unknown structural tier ${metadata.tier}`);
  }
  if (metadata.boundary !== "runtime-host-script") {
    throw new Error(
      `${entry.id}: current lane requires boundary=runtime-host-script`,
    );
  }
  for (const key of ["functions", "modules", "hostBoundaries", "callDepth"]) {
    if (
      !Number.isInteger(metadata.structure?.[key]) ||
      metadata.structure[key] < 1
    ) {
      throw new Error(
        `${entry.id}: structure.${key} must be a positive integer`,
      );
    }
  }
  const tierRanges = {
    "small-structural": [3, 7],
    "medium-structural": [8, 19],
    "large-structural": [20, Infinity],
  };
  const [minimumFunctions, maximumFunctions] = tierRanges[metadata.tier];
  if (
    metadata.structure.functions < minimumFunctions ||
    metadata.structure.functions > maximumFunctions
  ) {
    throw new Error(
      `${entry.id}: ${metadata.tier} requires ${minimumFunctions}` +
        `${Number.isFinite(maximumFunctions) ? `-${maximumFunctions}` : "+"} functions`,
    );
  }
  if (metadata.tier === "large-structural" && metadata.structure.modules < 6) {
    throw new Error(
      `${entry.id}: large-structural requires at least 6 source modules`,
    );
  }
  const lilModuleNames = Object.keys(entry.lilSources)
    .map((name) => name.replace(/\.lil$/u, ""))
    .sort();
  const javascriptModuleNames = Object.keys(entry.javascriptSources)
    .map((name) => name.replace(/\.js$/u, ""))
    .sort();
  const lilModules = lilModuleNames.length;
  const javascriptModules = javascriptModuleNames.length;
  if (
    metadata.structure.modules !== lilModules ||
    metadata.structure.modules !== javascriptModules
  ) {
    throw new Error(
      `${entry.id}: structure.modules=${metadata.structure.modules}, ` +
        `but found ${lilModules} LilScript and ${javascriptModules} JavaScript modules`,
    );
  }
  if (
    JSON.stringify(lilModuleNames) !== JSON.stringify(javascriptModuleNames)
  ) {
    throw new Error(
      `${entry.id}: LilScript and JavaScript module names differ: ` +
        `${lilModuleNames.join(", ")} versus ${javascriptModuleNames.join(", ")}`,
    );
  }
  const lilFunctionNamesByModule = Object.fromEntries(
    Object.entries(entry.lilSources).map(([module, source]) => [
      module.replace(/\.lil$/u, ""),
      [
        ...source.matchAll(
          /^(?:export\s+)?(?:pure\s+)?[A-Za-z_]\w*(?:\[\])?\s+([A-Za-z_]\w*)\s*\([^;]*\)\s*\{/gm,
        ),
      ]
        .map((match) => match[1])
        .sort(),
    ]),
  );
  const lilImportsByModule = Object.fromEntries(
    Object.entries(entry.lilSources).map(([module, source]) => [
      module.replace(/\.lil$/u, ""),
      [...source.matchAll(/^\s*import\s+\{[^}]+\}\s+from\s+"([^"]+)"\s*;/gm)]
        .map((match) => match[1].replace(/^\.\//u, "").replace(/\.lil$/u, ""))
        .sort(),
    ]),
  );
  const lilFunctions = Object.values(lilFunctionNamesByModule).reduce(
    (count, names) => count + names.length,
    0,
  );
  const javascriptGraph = analyzeJavaScriptGraph(entry, parse);
  const javascriptFunctions = javascriptGraph.functions;
  if (
    metadata.structure.functions !== lilFunctions ||
    metadata.structure.functions !== javascriptFunctions
  ) {
    throw new Error(
      `${entry.id}: structure.functions=${metadata.structure.functions}, ` +
        `but found ${lilFunctions} LilScript and ${javascriptFunctions} JavaScript functions`,
    );
  }
  if (
    JSON.stringify(lilFunctionNamesByModule) !==
    JSON.stringify(javascriptGraph.functionNamesByModule)
  ) {
    throw new Error(
      `${entry.id}: LilScript and JavaScript function names or module ownership differ`,
    );
  }
  if (
    JSON.stringify(lilImportsByModule) !==
    JSON.stringify(javascriptGraph.importsByModule)
  ) {
    throw new Error(
      `${entry.id}: LilScript and JavaScript module dependency edges differ`,
    );
  }
  if (javascriptGraph.reachableFunctions.length < minimumFunctions) {
    throw new Error(
      `${entry.id}: ${metadata.tier} requires at least ${minimumFunctions} reachable functions, ` +
        `but only ${javascriptGraph.reachableFunctions.length}/${javascriptFunctions} are reachable`,
    );
  }
  if (javascriptGraph.reachableModules.length !== javascriptModules) {
    throw new Error(
      `${entry.id}: only ${javascriptGraph.reachableModules.length}/${javascriptModules} ` +
        `JavaScript modules contribute a reachable function`,
    );
  }
  if (metadata.structure.callDepth !== javascriptGraph.callDepth) {
    throw new Error(
      `${entry.id}: structure.callDepth=${metadata.structure.callDepth}, ` +
        `but JavaScript call graph depth is ${javascriptGraph.callDepth}`,
    );
  }
  const lilHostNames = new Set(
    Object.values(entry.lilSources).flatMap(
      (source) => source.match(/\balgorithm(?:Int|String|Count)\b/g) ?? [],
    ),
  );
  const javascriptHostNames = new Set(
    Object.values(entry.javascriptSources).flatMap(
      (source) => source.match(/\balgorithm(?:Int|String|Count)\b/g) ?? [],
    ),
  );
  const sortedLilHostNames = [...lilHostNames].sort();
  const sortedJavascriptHostNames = [...javascriptHostNames].sort();
  if (
    JSON.stringify(sortedLilHostNames) !==
    JSON.stringify(sortedJavascriptHostNames)
  ) {
    throw new Error(
      `${entry.id}: LilScript and JavaScript host boundaries differ: ` +
        `${sortedLilHostNames.join(", ")} versus ${sortedJavascriptHostNames.join(", ")}`,
    );
  }
  if (metadata.structure.hostBoundaries !== lilHostNames.size) {
    throw new Error(
      `${entry.id}: structure.hostBoundaries=${metadata.structure.hostBoundaries}, ` +
        `but found ${lilHostNames.size} distinct runtime host functions`,
    );
  }
  if (
    !Array.isArray(metadata.opportunities) ||
    metadata.opportunities.length === 0
  ) {
    throw new Error(`${entry.id}: opportunity tags are required`);
  }
  if (new Set(metadata.opportunities).size !== metadata.opportunities.length) {
    throw new Error(`${entry.id}: duplicate opportunity tag`);
  }
  if (
    javascriptGraph.reachableFunctions.length !== javascriptFunctions &&
    !metadata.opportunities.some((tag) =>
      [
        "dead-code-elimination",
        "export-reachability",
        "module-tree-shaking",
      ].includes(tag),
    )
  ) {
    throw new Error(
      `${entry.id}: unreachable functions require an explicit DCE or export-reachability opportunity`,
    );
  }
  const expectationKeys = Object.keys(metadata.expectations ?? {}).sort();
  if (JSON.stringify(expectationKeys) !== JSON.stringify([...metrics].sort())) {
    throw new Error(
      `${entry.id}: expectations must contain exactly raw, gzip9, and brotli11`,
    );
  }
  for (const metric of metrics) {
    if (!new Set(["le", "lt"]).has(metadata.expectations[metric])) {
      throw new Error(`${entry.id}: expectations.${metric} must be le or lt`);
    }
  }
  if (!Array.isArray(metadata.vectors) || metadata.vectors.length < 3) {
    throw new Error(
      `${entry.id}: at least 3 deterministic vectors are required`,
    );
  }
  const vectorNames = new Set();
  for (const vector of metadata.vectors) {
    if (
      typeof vector.name !== "string" ||
      !vector.name ||
      vectorNames.has(vector.name)
    ) {
      throw new Error(`${entry.id}: vector names must be non-empty and unique`);
    }
    vectorNames.add(vector.name);
    if (!Array.isArray(vector.ints) || !Array.isArray(vector.strings)) {
      throw new Error(
        `${entry.id}/${vector.name}: ints and strings arrays are required`,
      );
    }
    if (
      !vector.ints.every(
        (value) =>
          Number.isInteger(value) &&
          value >= -2147483648 &&
          value <= 2147483647,
      ) ||
      !vector.strings.every((value) => typeof value === "string")
    ) {
      throw new Error(
        `${entry.id}/${vector.name}: invalid runtime input value`,
      );
    }
    if (typeof vector.expected !== "string" || vector.expected.length === 0) {
      throw new Error(
        `${entry.id}/${vector.name}: fixed stdout oracle is required`,
      );
    }
    if (
      lilHostNames.has("algorithmCount") &&
      lilHostNames.has("algorithmInt") &&
      lilHostNames.has("algorithmString") &&
      vector.ints.length !== vector.strings.length
    ) {
      throw new Error(
        `${entry.id}/${vector.name}: mixed int/string streams must have equal lengths`,
      );
    }
  }
  return {
    ...entry,
    metadata,
    observedStructure: {
      functions: javascriptFunctions,
      reachableFunctions: javascriptGraph.reachableFunctions.length,
      modules: javascriptModules,
      reachableModules: javascriptGraph.reachableModules.length,
      callDepth: javascriptGraph.callDepth,
    },
  };
}

async function prepareJavaScript(entry, minifiers) {
  if (entry.metadata.structure.modules === 1) {
    return {
      ...entry,
      javascriptBundle: {
        kind: "single-script-source",
        digest: sha256(entry.js),
      },
    };
  }
  const result = await minifiers.esbuild.build({
    entryPoints: [entry.jsPath],
    ...baselineOptions.moduleBundling,
    write: false,
    logLevel: "silent",
  });
  if (result.outputFiles.length !== 1) {
    throw new Error(
      `${entry.id}: expected one JavaScript bundle, found ${result.outputFiles.length}`,
    );
  }
  const js = result.outputFiles[0].text;
  return {
    ...entry,
    js,
    javascriptBundle: {
      kind: "esbuild-unminified-iife",
      digest: sha256(js),
      bytes: Buffer.byteLength(js),
    },
  };
}

function validateCorpus(entries) {
  const required = {
    "static propagation/DCE": ["static-propagation", "dead-code-elimination"],
    "mangling/aggregate": ["property-mangling", "aggregate-layout"],
    "helper sharing": ["helper-sharing", "inlining"],
    "dictionary/repetition": ["dictionary-repetition", "string-pooling"],
    "collection/scalar replacement": [
      "array-builder-fusion",
      "scalar-replacement",
    ],
    "control flow/state machine": ["control-flow-structuring", "state-machine"],
  };
  const tags = new Set(
    entries.flatMap((entry) => entry.metadata.opportunities),
  );
  for (const [family, expected] of Object.entries(required)) {
    if (!expected.every((tag) => tags.has(tag))) {
      throw new Error(
        `algorithm corpus is missing ${family} opportunities: ${expected.join(", ")}`,
      );
    }
  }
  const tiers = new Set(entries.map((entry) => entry.metadata.tier));
  for (const tier of [
    "small-structural",
    "medium-structural",
    "large-structural",
  ]) {
    if (!tiers.has(tier))
      throw new Error(`algorithm corpus is missing tier ${tier}`);
  }
  for (const entry of entries) {
    entry.referenceVectorResults = [];
    entry.referenceHostAccessTraces = {};
    for (const vector of entry.metadata.vectors) {
      const started = performance.now();
      let execution;
      try {
        execution = execute(entry.js, vector);
      } catch (error) {
        throw new Error(
          `${entry.id}/${vector.name}: original JavaScript failed\n${error.message}`,
        );
      }
      if (execution.stdout !== vector.expected) {
        throw new Error(
          `${entry.id}/${vector.name}: fixed oracle drifted\n` +
            `expected ${JSON.stringify(vector.expected)}, got ${JSON.stringify(execution.stdout)}`,
        );
      }
      entry.referenceHostAccessTraces[vector.name] = execution.hostAccessTrace;
      entry.referenceVectorResults.push({
        vector: vector.name,
        valid: true,
        stdoutDigest: sha256(execution.stdout),
        hostAccessTrace: execution.hostAccessTrace,
        hostAccessTraceDigest: sha256(
          JSON.stringify(execution.hostAccessTrace),
        ),
        durationMs: performance.now() - started,
      });
    }
  }
}

function prepareCompiler() {
  if (!compilerOverride) {
    const cargo = process.env.CARGO ?? "cargo";
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
      { cwd: repo },
    );
  } else if (!existsSync(compiler)) {
    throw new Error(`LILSCRIPT does not exist: ${compiler}`);
  } else if (!existsSync(resolve(process.cwd(), codecOverride))) {
    throw new Error(`LILSCRIPT_CODEC does not exist: ${codecOverride}`);
  }
  run(compiler, ["--help"], { timeout: 30_000 });
  let version = "unreported";
  try {
    version = run(compiler, ["--version"], { timeout: 30_000 }).trim();
  } catch {}
  return {
    source: compilerOverride ? "LILSCRIPT" : "cargo-build-release",
    path: relative(repo, compiler) || compiler,
    version,
    digest: sha256(readFileSync(compiler)),
  };
}

async function loadMinifiers() {
  try {
    const { minify: terserMinify } = require("terser");
    const esbuild = require("esbuild");
    const { parse: acornParse } = require("acorn");
    const { build: viteBuild } = await import(
      pathToFileURL(join(popular, "node_modules/vite/dist/node/index.js"))
    );
    const { getNativeImagePath: getClosureNativeImagePath } = await import(
      pathToFileURL(
        join(popular, "node_modules/google-closure-compiler/lib/utils.js"),
      )
    );
    const { minifySync: oxcMinify } = await import(
      pathToFileURL(join(popular, "node_modules/rolldown/dist/utils-index.mjs"))
    );
    return {
      terserMinify,
      esbuild,
      viteBuild,
      oxcMinify,
      acornParse,
      closureNativePath: getClosureNativeImagePath(),
    };
  } catch (error) {
    throw new Error(
      `algorithm minifier dependencies are unavailable; run npm ci --prefix ${popular}\n${error.message}`,
    );
  }
}

async function buildBaselines(entry, minifiers, outputDirectory) {
  const viteBundle = async (id, minify) => {
    const viteOutputDirectory = join(outputDirectory, `${id}-output`);
    await minifiers.viteBuild({
      root: entry.directory,
      publicDir: false,
      configFile: false,
      logLevel: "silent",
      build: {
        lib: {
          entry: entry.jsPath,
          formats: ["iife"],
          name: "AlgorithmCase",
          fileName: "bundle",
        },
        outDir: viteOutputDirectory,
        emptyOutDir: true,
        copyPublicDir: false,
        minify,
        target: "es2022",
        sourcemap: false,
        ...(minify === "terser"
          ? {
              terserOptions: structuredClone(
                baselineOptions.viteTerserBundleIife.terserOptions,
              ),
            }
          : {}),
      },
    });
    const artifacts = filesUnder(viteOutputDirectory).filter((path) =>
      /\.(?:js|mjs)$/u.test(path),
    );
    if (artifacts.length !== 1) {
      throw new Error(
        `expected one Vite JavaScript artifact, found ${artifacts.length}`,
      );
    }
    return readFileSync(artifacts[0], "utf8");
  };
  const definitions = [
    {
      id: "terser",
      tool: "terser",
      inputKind: "prepared-executable",
      optionsKey: "terser",
      build: async () => {
        const result = await minifiers.terserMinify(
          entry.js,
          structuredClone(baselineOptions.terser),
        );
        if (!result.code)
          throw new Error(result.error?.message ?? "Terser returned no code");
        return result.code;
      },
    },
    {
      id: "oxc",
      tool: "oxc-via-rolldown",
      inputKind: "prepared-executable",
      optionsKey: "oxc",
      build: async () => {
        const result = minifiers.oxcMinify(
          `${entry.id}.js`,
          entry.js,
          structuredClone(baselineOptions.oxc),
        );
        if (result.errors?.length)
          throw new Error(JSON.stringify(result.errors));
        return result.code;
      },
    },
    {
      id: "closure-advanced",
      tool: "google-closure-compiler",
      inputKind: "prepared-executable",
      optionsKey: "closureAdvanced",
      build: async () => {
        const input = join(outputDirectory, "closure-advanced-input.js");
        const externs = join(outputDirectory, "closure-advanced-externs.js");
        const output = join(outputDirectory, "closure-advanced-output.js");
        writeFileSync(input, entry.js);
        writeFileSync(externs, closureExterns);
        run(
          process.execPath,
          [
            closureCliPath,
            "--platform=native,java",
            "--compilation_level=ADVANCED",
            "--language_in=ECMASCRIPT_2021",
            "--language_out=ECMASCRIPT_2021",
            "--env=BROWSER",
            "--warning_level=QUIET",
            "--rewrite_polyfills=false",
            "--assume_function_wrapper=true",
            `--js=${input}`,
            `--externs=${externs}`,
            `--js_output_file=${output}`,
          ],
          { timeout: 120_000 },
        );
        return readFileSync(output, "utf8");
      },
    },
    {
      id: "esbuild-script",
      tool: "esbuild",
      inputKind: "prepared-executable",
      optionsKey: "esbuildScript",
      build: async () =>
        (
          await minifiers.esbuild.transform(
            entry.js,
            structuredClone(baselineOptions.esbuildScript),
          )
        ).code,
    },
    {
      id: "esbuild-iife",
      tool: "esbuild",
      inputKind: "prepared-executable",
      optionsKey: "esbuildIife",
      build: async () =>
        (
          await minifiers.esbuild.transform(
            entry.js,
            structuredClone(baselineOptions.esbuildIife),
          )
        ).code,
    },
  ];
  if (entry.metadata.opportunities.includes("safe-property-mangling")) {
    definitions.push({
      id: "terser-properties",
      tool: "terser",
      inputKind: "prepared-executable",
      optionsKey: "terserProperties",
      build: async () => {
        const options = structuredClone(baselineOptions.terserProperties);
        const encodedRegex = options.mangle.properties.regex;
        options.mangle.properties.regex = new RegExp(
          encodedRegex.source,
          encodedRegex.flags,
        );
        const result = await minifiers.terserMinify(entry.js, options);
        if (!result.code)
          throw new Error(result.error?.message ?? "Terser returned no code");
        return result.code;
      },
    });
  }
  if (entry.metadata.structure.modules > 1) {
    definitions.push({
      id: "closure-advanced-module-graph",
      tool: "google-closure-compiler",
      inputKind: "javascript-module-graph",
      optionsKey: "closureAdvancedModuleGraph",
      build: async () => {
        const externs = join(
          outputDirectory,
          "closure-advanced-module-graph-externs.js",
        );
        const output = join(
          outputDirectory,
          "closure-advanced-module-graph-output.js",
        );
        writeFileSync(externs, closureExterns);
        run(
          process.execPath,
          [
            closureCliPath,
            "--platform=native,java",
            "--compilation_level=ADVANCED",
            "--dependency_mode=PRUNE",
            `--entry_point=${entry.jsPath}`,
            "--language_in=ECMASCRIPT_2021",
            "--language_out=ECMASCRIPT_2021",
            "--env=BROWSER",
            "--warning_level=QUIET",
            "--rewrite_polyfills=false",
            "--assume_function_wrapper=true",
            ...Object.keys(entry.javascriptSources)
              .sort()
              .map((name) => `--js=${join(entry.directory, name)}`),
            `--externs=${externs}`,
            `--js_output_file=${output}`,
          ],
          { timeout: 120_000 },
        );
        return readFileSync(output, "utf8");
      },
    });
    definitions.push({
      id: "esbuild-bundle-iife",
      tool: "esbuild",
      inputKind: "javascript-module-graph",
      optionsKey: "esbuildBundleIife",
      build: async () => {
        const result = await minifiers.esbuild.build({
          entryPoints: [entry.jsPath],
          ...baselineOptions.esbuildBundleIife,
          write: false,
          logLevel: "silent",
        });
        if (result.outputFiles.length !== 1) {
          throw new Error(
            `expected one minified bundle, found ${result.outputFiles.length}`,
          );
        }
        return result.outputFiles[0].text;
      },
    });
    definitions.push(
      {
        id: "vite-oxc-bundle-iife",
        tool: "vite-oxc",
        inputKind: "javascript-module-graph",
        optionsKey: "viteOxcBundleIife",
        build: async () => viteBundle("vite-oxc", "oxc"),
      },
      {
        id: "vite-terser-bundle-iife",
        tool: "vite-terser",
        inputKind: "javascript-module-graph",
        optionsKey: "viteTerserBundleIife",
        build: async () => viteBundle("vite-terser", "terser"),
      },
    );
  }
  const candidates = [];
  for (const definition of definitions) {
    const started = performance.now();
    try {
      const code = await definition.build();
      const artifact = join(outputDirectory, `${definition.id}.js`);
      writeFileSync(artifact, code);
      candidates.push({
        id: definition.id,
        tool: definition.tool,
        inputKind: definition.inputKind,
        optionsKey: definition.optionsKey,
        code,
        artifact: relative(repo, artifact),
        absoluteArtifact: artifact,
        digest: sha256(code),
        durationMs: performance.now() - started,
      });
    } catch (error) {
      candidates.push({
        id: definition.id,
        tool: definition.tool,
        inputKind: definition.inputKind,
        optionsKey: definition.optionsKey,
        error: error.message,
        durationMs: performance.now() - started,
      });
    }
  }
  const builtCandidates = candidates.filter(
    (candidate) => candidate.absoluteArtifact,
  );
  if (builtCandidates.length > 0) {
    const measurements = canonicalCodecMeasurementsForFiles(
      builtCandidates.map((candidate) => candidate.absoluteArtifact),
      `${entry.id} JavaScript baseline candidates`,
    );
    for (const [index, candidate] of builtCandidates.entries()) {
      candidate.sizes = reportSizes(measurements[index]);
      if (candidate.digest !== measurements[index].sha256) {
        throw new Error(
          `${entry.id}/${candidate.id}: artifact changed before measurement`,
        );
      }
      delete candidate.absoluteArtifact;
    }
  }
  return candidates;
}

function validateCandidate(candidate, vectors, referenceHostAccessTraces) {
  if (!candidate.code)
    return { valid: false, results: [], error: candidate.error };
  const results = [];
  for (const vector of vectors) {
    const started = performance.now();
    try {
      const execution = execute(candidate.code, vector);
      const expectedHostAccessTrace = referenceHostAccessTraces[vector.name];
      const stdoutMatches = execution.stdout === vector.expected;
      const traceMatches =
        JSON.stringify(execution.hostAccessTrace) ===
        JSON.stringify(expectedHostAccessTrace);
      results.push({
        vector: vector.name,
        valid: stdoutMatches && traceMatches,
        stdoutDigest: sha256(execution.stdout),
        hostAccessTraceDigest: sha256(
          JSON.stringify(execution.hostAccessTrace),
        ),
        durationMs: performance.now() - started,
        ...(stdoutMatches
          ? {}
          : { expected: vector.expected, actual: execution.stdout }),
        ...(traceMatches
          ? {}
          : {
              expectedHostAccessTrace,
              actualHostAccessTrace: execution.hostAccessTrace,
            }),
      });
    } catch (error) {
      results.push({
        vector: vector.name,
        valid: false,
        error: error.message,
        durationMs: performance.now() - started,
      });
    }
  }
  return { valid: results.every((result) => result.valid), results };
}

function installedRolldownBindings() {
  const scope = join(popular, "node_modules/@rolldown");
  if (!existsSync(scope)) return [];
  return readdirSync(scope)
    .filter(
      (name) =>
        name.startsWith("binding-") &&
        existsSync(join(scope, name, "package.json")),
    )
    .map((name) => {
      const pkg = JSON.parse(
        readFileSync(join(scope, name, "package.json"), "utf8"),
      );
      return { package: `@rolldown/${name}`, version: pkg.version };
    });
}

const minifiers = await loadMinifiers();
const closureRuntimePath =
  minifiers.closureNativePath ??
  join(popular, "node_modules/google-closure-compiler-java/compiler.jar");
const closureRuntime = {
  platform: minifiers.closureNativePath ? "native" : "java",
  path: relative(repo, closureRuntimePath),
  digest: sha256(readFileSync(closureRuntimePath)),
  version: run(
    process.execPath,
    [closureCliPath, "--platform=native,java", "--version"],
    { timeout: 30_000 },
  ).trim(),
};
const allEntries = [];
for (const entry of discoverCases().map((entry) =>
  validateCase(entry, minifiers.acornParse),
)) {
  allEntries.push(await prepareJavaScript(entry, minifiers));
}
validateCorpus(allEntries);
const corpusDigest = sha256(
  JSON.stringify(
    allEntries.map((entry) => ({
      metadata: entry.metadata,
      lilscript: entry.lilSources,
      javascript: entry.javascriptSources,
      executableJavaScriptDigest: entry.javascriptBundle.digest,
    })),
  ),
);
const selected = allEntries.filter((entry) =>
  only ? entry.id.includes(only) : true,
);
if (selected.length === 0)
  throw new Error(`no algorithm cases matched --only ${only}`);

const compilerProvenance = prepareCompiler();
requireCanonicalCodecRuntime("comparison/algorithms hard gate");
mkdirSync(buildRoot, { recursive: true });

const rows = [];
const failures = [];
for (const entry of selected) {
  const caseStarted = performance.now();
  const caseFailures = [];
  const outputDirectory = join(buildRoot, entry.id);
  mkdirSync(outputDirectory, { recursive: true });
  if (entry.javascriptBundle.kind === "esbuild-unminified-iife") {
    writeFileSync(join(outputDirectory, "original-bundle.js"), entry.js);
  }
  const baselineCandidates = await buildBaselines(
    entry,
    minifiers,
    outputDirectory,
  );
  for (const candidate of baselineCandidates) {
    const semantic = validateCandidate(
      candidate,
      entry.metadata.vectors,
      entry.referenceHostAccessTraces,
    );
    candidate.semanticValid = semantic.valid;
    candidate.vectorResults = semantic.results;
    delete candidate.code;
    if (!semantic.valid) {
      candidate.ineligibilityReason = candidate.error
        ? "build-failed"
        : "oracle-failed";
      const message = candidate.error
        ? `${entry.id}: ${candidate.id} build failed and is ineligible`
        : `${entry.id}: ${candidate.id} failed a stdout or host-access oracle`;
      caseFailures.push(message);
      failures.push(message);
    }
  }
  const validBaselines = baselineCandidates.filter(
    (candidate) => candidate.semanticValid,
  );
  if (validBaselines.length === 0) {
    const message = `${entry.id}: no semantically valid JavaScript baseline`;
    caseFailures.push(message);
    failures.push(message);
  }
  const winners = Object.fromEntries(
    metrics.map((metric) => {
      const winner = [...validBaselines].sort(
        (left, right) =>
          left.sizes[metric] - right.sizes[metric] ||
          left.sizes.raw - right.sizes.raw ||
          left.id.localeCompare(right.id) ||
          left.digest.localeCompare(right.digest),
      )[0];
      return [
        metric,
        winner
          ? {
              candidate: winner.id,
              tool: winner.tool,
              size: winner.sizes[metric],
            }
          : null,
      ];
    }),
  );

  const lilscript = {};
  const compiledLanes = [];
  for (const lane of lanes) {
    const started = performance.now();
    const artifact = join(outputDirectory, `lilscript-${lane.name}.js`);
    try {
      run(
        compiler,
        [
          entry.lilPath,
          "--config",
          lane.config,
          "--target",
          "js",
          "--mode",
          "production",
          "-o",
          artifact,
        ],
        { cwd: here, timeout: 120_000 },
      );
      const code = readFileSync(artifact, "utf8");
      const semantic = validateCandidate(
        { code },
        entry.metadata.vectors,
        entry.referenceHostAccessTraces,
      );
      compiledLanes.push({
        lane,
        artifact: relative(repo, artifact),
        absoluteArtifact: artifact,
        config: relative(repo, lane.config),
        digest: sha256(code),
        semanticValid: semantic.valid,
        vectorResults: semantic.results,
        durationMs: performance.now() - started,
      });
      if (!semantic.valid) {
        const message = `${entry.id}/${lane.name}: LilScript failed a stdout or host-access oracle`;
        caseFailures.push(message);
        failures.push(message);
      }
    } catch (error) {
      lilscript[lane.metric] = {
        artifact: relative(repo, artifact),
        config: relative(repo, lane.config),
        semanticValid: false,
        error: error.message,
        durationMs: performance.now() - started,
      };
      const message = `${entry.id}/${lane.name}: compile or execute failed`;
      caseFailures.push(message);
      failures.push(`${message}\n${error.message}`);
    }
  }
  try {
    const measurements =
      compiledLanes.length === 0
        ? []
        : canonicalCodecMeasurementsForFiles(
            compiledLanes.map(({ absoluteArtifact }) => absoluteArtifact),
            `${entry.id} LilScript objective artifacts`,
          );
    for (const [index, compiled] of compiledLanes.entries()) {
      const { lane, absoluteArtifact, ...result } = compiled;
      const measurement = measurements[index];
      result.sizes = reportSizes(measurement);
      if (result.digest !== measurement.sha256) {
        throw new Error(
          `${entry.id}/${lane.name}: artifact changed before measurement`,
        );
      }
      lilscript[lane.metric] = result;
      if (!result.semanticValid) continue;
      const winner = winners[lane.metric];
      if (!winner) continue;
      const actual = result.sizes[lane.metric];
      const expectation = entry.metadata.expectations[lane.metric];
      const missedGate =
        expectation === "lt" ? actual >= winner.size : actual > winner.size;
      if (missedGate) {
        const failedRelation = expectation === "lt" ? ">=" : ">";
        const message =
          `${entry.id}/${lane.name}: LilScript ${actual} ${failedRelation} ` +
          `${winner.candidate} ${winner.size} (expected ${expectation})`;
        caseFailures.push(message);
        failures.push(message);
      }
    }
  } catch (error) {
    const message = `${entry.id}: canonical LilScript measurement failed`;
    caseFailures.push(message);
    failures.push(`${message}\n${error.message}`);
  }

  rows.push({
    id: entry.id,
    title: entry.metadata.title,
    hypothesis: entry.metadata.hypothesis,
    tier: entry.metadata.tier,
    boundary: entry.metadata.boundary,
    structure: entry.metadata.structure,
    observedStructure: entry.observedStructure,
    opportunities: entry.metadata.opportunities,
    expectations: entry.metadata.expectations,
    vectors: entry.metadata.vectors.map((vector) => ({
      name: vector.name,
      inputDigest: sha256(
        JSON.stringify({ ints: vector.ints, strings: vector.strings }),
      ),
      expectedDigest: sha256(vector.expected),
    })),
    referenceVectorResults: entry.referenceVectorResults,
    sourceHashes: {
      metadata: sha256(entry.metadataSource),
      lilscriptModules: Object.fromEntries(
        Object.entries(entry.lilSources).map(([name, source]) => [
          name,
          sha256(source),
        ]),
      ),
      javascriptModules: Object.fromEntries(
        Object.entries(entry.javascriptSources).map(([name, source]) => [
          name,
          sha256(source),
        ]),
      ),
      executableJavaScript: sha256(entry.js),
    },
    javascriptPreparation: entry.javascriptBundle,
    baselineCandidates,
    winners,
    lilscript,
    sizeBands: {
      bestJavaScriptRaw: sizeBand(winners.raw?.size),
      lilscriptRaw: sizeBand(lilscript.raw?.sizes?.raw),
    },
    passed: caseFailures.length === 0,
    failures: caseFailures,
    durationMs: performance.now() - caseStarted,
  });
  console.log(
    `${entry.id}: ${caseFailures.length === 0 ? "passed" : `${caseFailures.length} failures`}`,
  );
}

const passedCases = rows.filter((row) => row.passed).length;
const report = {
  schemaVersion: 2,
  catalogCases: allEntries.length,
  selectedBy: only ?? "all",
  cases: rows.length,
  passedCases,
  failedCases: rows.length - passedCases,
  failureEvents: failures.length,
  failureDetails: failures,
  gate: "Each LilScript metric lane must satisfy its manifest le/lt relation against the independently minimized valid JavaScript baseline for that metric",
  lilscriptLaneContract: Object.fromEntries(
    lanes.map((lane) => [
      lane.name,
      {
        independentCompilation: true,
        gateMetric: lane.metric,
        diagnosticMetrics: metrics.filter((metric) => metric !== lane.metric),
        config: relative(repo, lane.config),
      },
    ]),
  ),
  codecs: canonicalCodecProvenance("comparison/algorithms report"),
  runtime: {
    node: process.versions.node,
    v8: process.versions.v8,
    platform: platform(),
    arch: arch(),
    nodeCodecs: {
      classification: "diagnostic-only; hard gates use lilscript-codec",
      zlib: process.versions.zlib,
      brotli: process.versions.brotli,
    },
  },
  provenance: {
    corpusDigest,
    runnerDigest: sha256(readFileSync(fileURLToPath(import.meta.url))),
    hostDigest: sha256(readFileSync(hostPath)),
    closureExternsDigest: sha256(closureExterns),
    toolLockfile: {
      path: relative(repo, join(popular, "package-lock.json")),
      digest: sha256(readFileSync(join(popular, "package-lock.json"))),
    },
    configs: Object.fromEntries(
      lanes.map((lane) => [
        lane.name,
        {
          path: relative(repo, lane.config),
          digest: sha256(readFileSync(lane.config)),
        },
      ]),
    ),
  },
  baselineOptions,
  toolVersions: {
    lilscript: compilerProvenance,
    terser: require("terser/package.json").version,
    rolldown: require("rolldown/package.json").version,
    rolldownBindings: installedRolldownBindings(),
    googleClosureCompiler: require("google-closure-compiler/package.json")
      .version,
    googleClosureCompilerRuntime: closureRuntime,
    esbuild: require("esbuild/package.json").version,
    vite: require("vite/package.json").version,
    acorn: require("acorn/package.json").version,
  },
  rows,
};
writeFileSync(summaryJsonPath, `${JSON.stringify(report, null, 2)}\n`);

const header =
  "| Algorithm | Tier | Functions/modules/depth | Lil raw relation JS | Lil gzip relation JS | Lil Brotli relation JS | Gate |\n" +
  "|---|---|---:|---:|---:|---:|---|\n";
const table = rows
  .map((row) => {
    const cell = (metric) => {
      const lil = row.lilscript[metric]?.sizes?.[metric] ?? "error";
      const winner = row.winners[metric];
      const symbol = row.expectations[metric] === "lt" ? "<" : "≤";
      return `${lil} ${symbol} ${winner?.size ?? "error"} (${winner?.candidate ?? "none"})`;
    };
    return `| ${row.id} | ${row.tier} | ${row.structure.functions}/${row.structure.modules}/${row.structure.callDepth} | ${cell("raw")} | ${cell("gzip9")} | ${cell("brotli11")} | ${row.passed ? "pass" : "FAIL"} |`;
  })
  .join("\n");
writeFileSync(
  summaryMarkdownPath,
  `# Escalating algorithm compression corpus\n\n` +
    `${rows.length} selected algorithms; ${passedCases} passed and ${rows.length - passedCases} failed with ${failures.length} failure events.\n\n` +
    `${header}${table}\n`,
);

if (failures.length > 0) {
  console.error(`\n${failures.join("\n\n")}\n`);
  console.error(
    `${rows.length - passedCases}/${rows.length} algorithm cases failed; see comparison/algorithms/summary.md`,
  );
  process.exit(1);
}

console.log(
  `Algorithm compression corpus passed: ${rows.length}/${rows.length}`,
);
