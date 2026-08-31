import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { extname, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import vue from "@vitejs/plugin-vue";
import { build } from "vite";
import { projectRoot, scopePath } from "./audit-scope.mjs";

const outputRoot = resolve(projectRoot, "artifacts/generated/project-comparison");
const reportPath = resolve(outputRoot, "build-report.json");
const packageRoot = resolve(projectRoot, "packages/vuelil");
const productionRoot = resolve(packageRoot, "production");
const openWorldConfig = resolve(projectRoot, "config/open-world.toml");
const vueRuntime = resolve(productionRoot, "vue.runtime.js");
const vueCompiler = resolve(productionRoot, "vue.js");
const vueServerRenderer = resolve(productionRoot, "server-renderer.js");
const upstreamCompiler = resolve(
  projectRoot,
  "node_modules/vue/dist/vue.esm-bundler.js",
);

const defines = Object.freeze({
  __VUE_OPTIONS_API__: "false",
  __VUE_PROD_DEVTOOLS__: "false",
  __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: "false",
  "process.env.NODE_ENV": '"production"',
});

const commonBuildConfig = Object.freeze({
  mode: "production",
  target: "es2022",
  minify: "oxc",
  sourcemap: false,
  modulePreload: false,
  cssCodeSplit: true,
  reportCompressedSize: false,
  emptyOutDir: false,
  defines,
  output: {
    format: "es",
    entryFileNames: "assets/app.js",
    chunkFileNames: "assets/chunks/[name].js",
    assetFileNames: "assets/[name][extname]",
    codeSplitting: false,
  },
  treeShaking: true,
});

const scenarios = Object.freeze([
  {
    id: "runtime-only-client",
    kind: "browser",
    root: resolve(projectRoot, "apps/runtime-only-client"),
    entry: "index.html",
    candidateAliases: [["vue", vueRuntime]],
    adapters: ["reactivity", "runtime-core", "runtime-dom"],
  },
  {
    id: "runtime-compiler-client",
    kind: "browser",
    root: resolve(projectRoot, "apps/runtime-compiler-client"),
    entry: "index.html",
    upstreamAliases: [["vue", upstreamCompiler]],
    candidateAliases: [["vue", vueCompiler]],
    adapters: ["reactivity", "runtime-core", "runtime-dom", "compiler-dom"],
  },
  {
    id: "ssr-app",
    kind: "ssr",
    root: resolve(projectRoot, "apps/ssr-app"),
    entry: "src/main.js",
    candidateAliases: [
      ["vue/server-renderer", vueServerRenderer],
      ["vue", vueRuntime],
    ],
    adapters: ["reactivity", "runtime-core", "runtime-dom", "server-renderer"],
  },
  {
    id: "sfc-production-app",
    kind: "sfc",
    root: resolve(projectRoot, "apps/sfc-production-app"),
    entry: "index.html",
    candidateAliases: [["vue", vueRuntime]],
    adapters: ["reactivity", "runtime-core", "runtime-dom"],
  },
]);

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function projectPath(path) {
  return relative(projectRoot, path).split(sep).join("/");
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function fileEvidence(path) {
  const bytes = readFileSync(path);
  return { path: projectPath(path), bytes: bytes.length, sha256: sha256(bytes) };
}

function jsonPackage(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function buildCandidateModules() {
  const builds = [
    "build-shared.mjs",
    "build-reactivity.mjs",
    "build-runtime-core.mjs",
    "build-runtime-dom.mjs",
    "build-compiler-core.mjs",
    "build-compiler-dom.mjs",
    "build-vue.mjs",
    "build-compiler-ssr.mjs",
    "build-server-renderer.mjs",
  ];
  for (const script of builds) {
    const env = script === "build-reactivity.mjs"
      ? { ...process.env, VUELIL_SKIP_REACTIVITY_REPORT: "1" }
      : process.env;
    const result = spawnSync(process.execPath, [resolve(projectRoot, "scripts", script)], {
      cwd: projectRoot,
      encoding: "utf8",
      env,
      maxBuffer: 64 * 1024 * 1024,
    });
    if (result.status !== 0) {
      throw new Error(
        `Unable to build reusable VueLil production modules with ${script}: ${(
          result.stderr || result.stdout || `exit ${result.status}`
        ).trim()}`,
      );
    }
  }
}

function sourceFiles(root) {
  const files = [];
  function visit(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if ([".html", ".js", ".vue"].includes(extname(entry.name))) files.push(path);
    }
  }
  visit(root);
  return files.sort((left, right) => compareText(projectPath(left), projectPath(right)));
}

function inputEvidence(scenario) {
  const files = sourceFiles(scenario.root).map(fileEvidence);
  const manifest = files
    .map(({ path, bytes, sha256: digest }) => `${path}\0${bytes}\0${digest}\n`)
    .join("");
  const importSpecifiers = [...new Set(files.flatMap((entry) => {
    const source = readFileSync(resolve(projectRoot, entry.path), "utf8");
    return [...source.matchAll(/\bfrom\s+["'](vue(?:\/[^"']*)?)["']/gu)]
      .map((match) => match[1]);
  }))].sort(compareText);
  return {
    entry: projectPath(resolve(scenario.root, scenario.entry)),
    importSpecifier: "vue",
    importSpecifiers,
    files,
    sha256: sha256(manifest),
  };
}

function normalizeModuleId(id) {
  const nul = id.startsWith("\0");
  const clean = nul ? id.slice(1) : id;
  const queryIndex = clean.indexOf("?");
  const path = queryIndex === -1 ? clean : clean.slice(0, queryIndex);
  const query = queryIndex === -1 ? "" : clean.slice(queryIndex);
  if (!path.startsWith("/")) return `${nul ? "virtual:" : ""}${clean}`;
  const normalized = projectPath(path);
  return `${normalized.startsWith("../") ? path : normalized}${query}`;
}

function aliases(entries = []) {
  return entries.map(([specifier, replacement]) => ({
    find: new RegExp(`^${specifier.replace("/", "\\/")}$`, "u"),
    replacement,
  }));
}

function serializedAliases(entries = []) {
  return entries.map(([specifier, replacement]) => ({
    specifier,
    target: projectPath(replacement),
    targetSha256: fileEvidence(replacement).sha256,
  }));
}

function adapterEvidence(names) {
  return names.map((name) => {
    const path = resolve(projectRoot, `src/${name}/host.js`);
    return {
      ...fileEvidence(path),
      owner: name,
      accounting: "inlined into VueLil before Vite; all retained bytes are measured in emitted assets",
    };
  });
}

function executionRunner(kind) {
  const publish = [
    'const result = globalThis.__VUELIL_PROJECT_RESULT__;',
    'if (typeof result !== "string") throw new Error("bundle did not publish a result");',
    "process.stdout.write(result);",
  ];
  if (kind === "ssr") {
    return ["await import(process.argv[1]);", ...publish].join("");
  }
  return [
    'const { JSDOM } = await import("jsdom");',
    'const dom = new JSDOM("<!doctype html><html><body><div id=app></div></body></html>", { url: "https://vuelil.test/" });',
    "const window = dom.window;",
    "for (const name of ['window','document','navigator','Node','Element','HTMLElement','SVGElement','MathMLElement','Document','ShadowRoot','Event','CustomEvent','MutationObserver']) { if (window[name] !== undefined) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] }); }",
    "await import(process.argv[1]);",
    ...publish,
  ].join("");
}

function executeBundle(path, kind) {
  const execution = spawnSync(
    "npx",
    [
      "--yes",
      "node@24",
      "--input-type=module",
      "--eval",
      executionRunner(kind),
      pathToFileURL(path).href,
    ],
    { cwd: projectRoot, encoding: "utf8", maxBuffer: 4 * 1024 * 1024 },
  );
  if (execution.status !== 0 || execution.stderr !== "") {
    throw new Error(
      `Unable to execute ${projectPath(path)}: ${(
        execution.stderr || execution.stdout || `exit ${execution.status}`
      ).trim()}`,
    );
  }
  return {
    result: execution.stdout,
    checksum: sha256(execution.stdout),
    expected: true,
    runtime: "node@24",
  };
}

function writeOutputs(root, outputs) {
  const files = [];
  for (const output of outputs) {
    const path = resolve(root, output.fileName);
    mkdirSync(resolve(path, ".."), { recursive: true });
    const bytes = output.type === "chunk" ? output.code : output.source;
    writeFileSync(path, bytes);
    files.push(fileEvidence(path));
  }
  files.sort((left, right) => compareText(left.path, right.path));
  const manifest = files
    .map(({ path, bytes, sha256: digest }) => `${path}\0${bytes}\0${digest}\n`)
    .join("");
  return {
    files,
    bytes: files.reduce((sum, file) => sum + file.bytes, 0),
    sha256: sha256(manifest),
  };
}

async function buildVariant(scenario, name) {
  const candidate = name === "candidate";
  const aliasEntries = candidate
    ? scenario.candidateAliases
    : scenario.upstreamAliases ?? [];
  const plugins = scenario.kind === "sfc" ? [vue()] : [];
  const scenarioConfig = {
    kind: scenario.kind,
    plugins: plugins.map((plugin) => plugin.name),
    ssrNoExternal: scenario.kind === "ssr",
  };
  const result = await build({
    configFile: false,
    root: scenario.root,
    mode: commonBuildConfig.mode,
    logLevel: "silent",
    plugins,
    resolve: { alias: aliases(aliasEntries) },
    define: { ...defines },
    ssr: scenario.kind === "ssr" ? { noExternal: true } : undefined,
    build: {
      write: false,
      target: commonBuildConfig.target,
      minify: commonBuildConfig.minify,
      sourcemap: commonBuildConfig.sourcemap,
      modulePreload: commonBuildConfig.modulePreload,
      cssCodeSplit: commonBuildConfig.cssCodeSplit,
      reportCompressedSize: commonBuildConfig.reportCompressedSize,
      emptyOutDir: commonBuildConfig.emptyOutDir,
      ssr: scenario.kind === "ssr" ? resolve(scenario.root, scenario.entry) : false,
      rolldownOptions: {
        treeshake: commonBuildConfig.treeShaking,
        output: { ...commonBuildConfig.output },
      },
    },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap(
    (entry) => entry.output,
  );
  const chunks = outputs.filter((entry) => entry.type === "chunk");
  const entryChunk = chunks.find((entry) => entry.isEntry);
  if (!entryChunk || chunks.length !== 1) {
    throw new Error(`${scenario.id}/${name} emitted ${chunks.length} chunks; expected one`);
  }

  const variantRoot = resolve(outputRoot, scenario.id, name);
  mkdirSync(variantRoot, { recursive: true });
  const artifact = writeOutputs(variantRoot, outputs);
  artifact.entry = projectPath(resolve(variantRoot, entryChunk.fileName));
  const modules = Object.entries(entryChunk.modules)
    .map(([id, details]) => ({
      id: normalizeModuleId(id),
      renderedBytes: details.renderedLength,
    }))
    .sort((left, right) => compareText(left.id, right.id));
  const upstreamRuntimeModules = modules
    .map(({ id }) => id)
    .filter(
      (id) =>
        id.includes("node_modules/vue/") ||
        id.includes("node_modules/@vue/") ||
        id.includes("upstream/vue/"),
    );
  const candidateModules = modules
    .map(({ id }) => id)
    .filter((id) => id.split("?", 1)[0].startsWith("packages/vuelil/"));
  const scenarioSpecificCandidateModules = candidateModules.filter(
    (id) => !/^packages\/vuelil\/production\/[^/]+\.js$/u.test(id.split("?", 1)[0]),
  );
  if (
    candidate &&
    (
      candidateModules.length === 0 ||
      upstreamRuntimeModules.length !== 0 ||
      scenarioSpecificCandidateModules.length !== 0
    )
  ) {
    throw new Error(
      `${scenario.id} candidate module audit failed: VueLil=${candidateModules.length}, ` +
        `upstream=${upstreamRuntimeModules.join(", ")}, ` +
        `non-reusable=${scenarioSpecificCandidateModules.join(", ")}`,
    );
  }
  if (!candidate && upstreamRuntimeModules.length === 0) {
    throw new Error(`${scenario.id} upstream build did not bundle the installed Vue runtime`);
  }
  return {
    resolution: {
      kind: candidate ? "vuelil-package-alias" : "installed-vue-package",
      aliases: serializedAliases(aliasEntries),
    },
    artifact,
    execution: executeBundle(resolve(projectRoot, artifact.entry), scenario.kind),
    buildConfigSha256: sha256(
      Buffer.from(`${JSON.stringify({ commonBuildConfig, scenarioConfig })}\n`),
    ),
    moduleGraph: {
      sha256: sha256(Buffer.from(`${JSON.stringify(modules)}\n`)),
      modules,
    },
    audit: {
      includesCandidateModule: candidateModules.length > 0,
      candidateModules,
      scenarioSpecificCandidateModules,
      noScenarioSpecificCandidateModules:
        scenarioSpecificCandidateModules.length === 0,
      upstreamRuntimeModules,
      noUpstreamRuntime: upstreamRuntimeModules.length === 0,
      reusableProductionPackage: candidate,
      viteDownstreamTreeShaking: commonBuildConfig.treeShaking,
      adapters: candidate ? adapterEvidence(scenario.adapters) : [],
      allEmittedAdapterCodeCounted: candidate,
    },
  };
}

export async function buildProjectComparison() {
  const scope = jsonPackage(scopePath);
  const openWorldSource = readFileSync(openWorldConfig, "utf8");
  for (const setting of [
    "identifiers = true",
    "properties = true",
    "exports = false",
  ]) {
    if (!openWorldSource.includes(setting)) {
      throw new Error(`open-world production config is missing ${setting}`);
    }
  }
  const scopedScenarios = new Map(
    scope.bundleScenarios
      .filter(({ completionRequired }) => completionRequired)
      .map((scenario) => [scenario.id, scenario]),
  );
  if (
    scopedScenarios.size !== scenarios.length ||
    scenarios.some(({ id }) => !scopedScenarios.has(id))
  ) {
    throw new Error("project harness must exactly match the four completion-required scenarios");
  }
  if (process.env.VUELIL_REUSE_PRODUCTION_MODULES !== "1") {
    buildCandidateModules();
  }

  const vitePackagePath = resolve(projectRoot, "node_modules/vite/package.json");
  const pluginPackagePath = resolve(
    projectRoot,
    "node_modules/@vitejs/plugin-vue/package.json",
  );
  const rolldownPackagePath = resolve(projectRoot, "node_modules/rolldown/package.json");
  const oxcPackagePath = resolve(
    projectRoot,
    "node_modules/@oxc-project/types/package.json",
  );
  const vuePackagePath = resolve(projectRoot, "node_modules/vue/package.json");
  const lockPath = resolve(projectRoot, "package-lock.json");
  const vitePackage = jsonPackage(vitePackagePath);
  const pluginPackage = jsonPackage(pluginPackagePath);
  const rolldownPackage = jsonPackage(rolldownPackagePath);
  const oxcPackage = jsonPackage(oxcPackagePath);
  const vuePackage = jsonPackage(vuePackagePath);
  const lock = jsonPackage(lockPath);
  if (vitePackage.version !== "8.2.1") {
    throw new Error(`project comparison requires pinned Vite 8.2.1, found ${vitePackage.version}`);
  }
  if (vuePackage.version !== scope.upstream.version) {
    throw new Error(
      `installed vue ${vuePackage.version} does not match scope ${scope.upstream.version}`,
    );
  }

  rmSync(outputRoot, { recursive: true, force: true });
  mkdirSync(outputRoot, { recursive: true });
  const results = [];
  for (const scenario of scenarios) {
    const input = inputEvidence(scenario);
    if (!input.importSpecifiers.includes("vue")) {
      throw new Error(`${scenario.id} must import from the public vue specifier`);
    }
    const upstream = await buildVariant(scenario, "upstream");
    const candidate = await buildVariant(scenario, "candidate");
    if (upstream.execution.result !== candidate.execution.result) {
      throw new Error(`${scenario.id} paired bundles returned different deterministic results`);
    }
    if (upstream.buildConfigSha256 !== candidate.buildConfigSha256) {
      throw new Error(`${scenario.id} paired bundles used different build settings`);
    }
    results.push({
      scenario: {
        id: scenario.id,
        completionRequired: true,
        description: scopedScenarios.get(scenario.id).description,
      },
      input,
      variants: { upstream, candidate },
      comparison: {
        sameInput: true,
        sameBuildConfig: true,
        onlyModuleResolutionChanged: true,
        deterministicChecksum: upstream.execution.checksum,
        matchingExecution: true,
        candidateNoUpstreamRuntime: true,
        passed: true,
      },
    });
    console.log(
      `Built ${scenario.id} with matching checksum ${upstream.execution.checksum}.`,
    );
  }

  const report = {
    schemaVersion: 3,
    generatedBy: "scripts/build-project-comparison.mjs",
    productionPackage: {
      root: projectPath(productionRoot),
      reusedPrebuiltModules: process.env.VUELIL_REUSE_PRODUCTION_MODULES === "1",
      config: fileEvidence(openWorldConfig),
      exportsPreserved: true,
      identifiersMangled: true,
      propertiesMangled: true,
      scenarioSpecificBuildInputs: false,
      viteDownstreamTreeShaking: true,
      entries: {
        runtime: fileEvidence(vueRuntime),
        compiler: fileEvidence(vueCompiler),
        serverRenderer: fileEvidence(vueServerRenderer),
      },
    },
    upstream: {
      package: "vue",
      version: scope.upstream.version,
      revision: scope.upstream.revision,
      npmIntegrity: lock.packages?.["node_modules/vue"]?.integrity ?? null,
      packageJson: fileEvidence(vuePackagePath),
    },
    toolchain: {
      node: process.version,
      executionNode: "node@24",
      vite: { version: vitePackage.version, packageJson: fileEvidence(vitePackagePath) },
      viteVuePlugin: {
        version: pluginPackage.version,
        packageJson: fileEvidence(pluginPackagePath),
      },
      bundler: {
        name: "rolldown",
        version: rolldownPackage.version,
        packageJson: fileEvidence(rolldownPackagePath),
      },
      minifier: {
        name: "oxc",
        version: oxcPackage.version,
        integration: "Rolldown embedded Oxc Minifier",
        versionSource: fileEvidence(oxcPackagePath),
      },
      packageLock: fileEvidence(lockPath),
      config: commonBuildConfig,
      commonConfigSha256: sha256(
        Buffer.from(`${JSON.stringify(commonBuildConfig)}\n`),
      ),
    },
    scenarios: results,
  };
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === import.meta.filename) {
  await buildProjectComparison();
}
