import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "vite";
import { buildSelectedReactivity } from "./build-reactivity.mjs";
import { projectRoot, scopePath } from "./audit-scope.mjs";
import { staticImportedNames } from "../src/reactivity/build-entry.mjs";

const scenarioId = "reactivity-app";
const appRoot = resolve(projectRoot, "apps/reactivity");
const appEntry = resolve(appRoot, "index.html");
const appSource = resolve(appRoot, "src/main.js");
const candidateModule = resolve(
  projectRoot,
  "artifacts/reactivity.esm-browser.prod.js",
);
const reactivitySource = resolve(projectRoot, "src/reactivity/index.lil");
const hostAdapter = resolve(projectRoot, "src/reactivity/host.js");
const productionConfig = resolve(projectRoot, "config/reactivity-production.toml");
const outputRoot = resolve(
  projectRoot,
  "artifacts/generated/project-comparison/reactivity-app",
);
const selectedCandidateModule = resolve(outputRoot, "integration/reactivity.js");
const reportPath = resolve(outputRoot, "build-report.json");

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
  defines,
  output: {
    format: "es",
    entryFileNames: "assets/app.js",
    chunkFileNames: "assets/chunks/[name]-[hash].js",
    assetFileNames: "assets/[name]-[hash][extname]",
    codeSplitting: false,
  },
});

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

function inputEvidence() {
  const files = [appEntry, appSource]
    .map(fileEvidence)
    .sort((left, right) => compareText(left.path, right.path));
  const manifest = files
    .map(({ path, bytes, sha256: digest }) => `${path}\0${bytes}\0${digest}\n`)
    .join("");
  return {
    entry: projectPath(appEntry),
    source: projectPath(appSource),
    importSpecifier: "vue",
    files,
    sha256: sha256(manifest),
  };
}

function normalizeModuleId(id) {
  const clean = id.replace(/^\0/u, "virtual:").split("?")[0];
  if (!clean.startsWith("/")) return clean;
  const path = projectPath(clean);
  return path.startsWith("../") ? clean : path;
}

function executeBundle(path) {
  const runner = [
    "await import(process.argv[1]);",
    'const result = globalThis.__VUELIL_REACTIVITY_RESULT__;',
    'if (typeof result !== "string") throw new Error("bundle did not publish a result");',
    "process.stdout.write(result);",
  ].join("");
  const execution = spawnSync(
    process.execPath,
    ["--input-type=module", "--eval", runner, pathToFileURL(path).href],
    { cwd: projectRoot, encoding: "utf8", maxBuffer: 1024 * 1024 },
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
  };
}

async function buildVariant(name, integration, useSelectedArtifact = false) {
  const candidate = name !== "upstream";
  const candidateCode = useSelectedArtifact
    ? readFileSync(selectedCandidateModule, "utf8")
    : null;
  const result = await build({
    configFile: false,
    root: appRoot,
    mode: commonBuildConfig.mode,
    logLevel: "silent",
    resolve: candidate
      ? { alias: [{ find: /^vue$/u, replacement: candidateModule }] }
      : undefined,
    plugins: useSelectedArtifact
      ? [{
          name: "vuelil-closed-world-production-integration",
          enforce: "pre",
          load(id) {
            if (id.split("?", 1)[0] === candidateModule) {
              return { code: candidateCode, map: null };
            }
            return null;
          },
        }]
      : undefined,
    define: { ...defines },
    build: {
      write: false,
      target: commonBuildConfig.target,
      minify: commonBuildConfig.minify,
      sourcemap: commonBuildConfig.sourcemap,
      modulePreload: commonBuildConfig.modulePreload,
      cssCodeSplit: commonBuildConfig.cssCodeSplit,
      reportCompressedSize: commonBuildConfig.reportCompressedSize,
      rolldownOptions: { output: { ...commonBuildConfig.output } },
    },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap(
    (entry) => entry.output,
  );
  const chunks = outputs.filter((entry) => entry.type === "chunk");
  const chunk = chunks.find((entry) => entry.isEntry);
  if (!chunk || chunks.length !== 1) {
    throw new Error(`${name} build emitted ${chunks.length} chunks; expected one entry bundle`);
  }

  const variantRoot = resolve(outputRoot, name);
  const artifactPath = resolve(variantRoot, "app.js");
  mkdirSync(variantRoot, { recursive: true });
  writeFileSync(artifactPath, chunk.code);

  const modules = Object.entries(chunk.modules)
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
  const includesCandidateModule = modules.some(
    ({ id }) => id === projectPath(candidateModule),
  );
  const candidateRenderedBytes = modules.find(
    ({ id }) => id === projectPath(candidateModule),
  )?.renderedBytes ?? 0;
  if (candidate && (!includesCandidateModule || upstreamRuntimeModules.length !== 0)) {
    throw new Error(
      `candidate module audit failed: includesCandidate=${includesCandidateModule}, upstream=${upstreamRuntimeModules.join(", ")}`,
    );
  }
  if (!candidate && upstreamRuntimeModules.length === 0) {
    throw new Error("upstream build did not include an installed Vue runtime module");
  }

  return {
    resolution: candidate
      ? {
          kind: "source-derived-closed-world-alias",
          specifier: "vue",
          target: projectPath(candidateModule),
          targetSha256: fileEvidence(candidateModule).sha256,
          ...(useSelectedArtifact ? { integration } : {}),
        }
      : {
          kind: "vite-package-resolution",
          specifier: "vue",
          target: "vue@3.5.42",
        },
    artifact: fileEvidence(artifactPath),
    execution: executeBundle(artifactPath),
    moduleGraph: {
      sha256: sha256(Buffer.from(`${JSON.stringify(modules)}\n`)),
      modules,
    },
    audit: {
      includesCandidateModule,
      upstreamRuntimeModules,
      noUpstreamRuntime: upstreamRuntimeModules.length === 0,
      ...(useSelectedArtifact
        ? {
            hostAdapterAccounting: {
              sourceBytes: integration.hostAdapter.bytes,
              selectedModuleRenderedBytes: candidateRenderedBytes,
              emittedJavaScriptBytes: Buffer.byteLength(chunk.code),
              emittedJavaScriptChunks: chunks.length,
              adapterExternalized: false,
              allEmittedAdapterCodeCounted: chunks.length === 1,
            },
          }
        : {}),
    },
  };
}

export async function buildProjectComparison() {
  const scope = jsonPackage(scopePath);
  const scenario = scope.bundleScenarios?.find(({ id }) => id === scenarioId);
  if (!scenario || scenario.completionRequired !== false) {
    throw new Error(`${scenarioId} must be declared as a diagnostic bundle scenario`);
  }
  const sourceText = readFileSync(appSource, "utf8");
  if (!/from\s+["']vue["']/u.test(sourceText)) {
    throw new Error(`${projectPath(appSource)} must import from the public vue specifier`);
  }

  const vitePackagePath = resolve(projectRoot, "node_modules/vite/package.json");
  const rolldownPackagePath = resolve(projectRoot, "node_modules/rolldown/package.json");
  const oxcPackagePath = resolve(
    projectRoot,
    "node_modules/@oxc-project/types/package.json",
  );
  const vuePackagePath = resolve(projectRoot, "node_modules/vue/package.json");
  const lockPath = resolve(projectRoot, "package-lock.json");
  const lexerPackagePath = resolve(projectRoot, "node_modules/es-module-lexer/package.json");
  const vitePackage = jsonPackage(vitePackagePath);
  const rolldownPackage = jsonPackage(rolldownPackagePath);
  const oxcPackage = jsonPackage(oxcPackagePath);
  const vuePackage = jsonPackage(vuePackagePath);
  const lock = jsonPackage(lockPath);
  const lexerPackage = jsonPackage(lexerPackagePath);
  if (vitePackage.version !== "8.2.1" || !vitePackage.version.startsWith("8.")) {
    throw new Error(`project comparison requires pinned Vite 8.2.1, found ${vitePackage.version}`);
  }
  if (vuePackage.version !== scope.upstream.version) {
    throw new Error(
      `installed vue ${vuePackage.version} does not match scope ${scope.upstream.version}`,
    );
  }

  rmSync(outputRoot, { recursive: true, force: true });
  mkdirSync(outputRoot, { recursive: true });
  const staticExportNames = await staticImportedNames(sourceText, "vue");
  const selected = await buildSelectedReactivity(staticExportNames, selectedCandidateModule);
  const integration = {
    kind: "lilscript-static-export-selection",
    applicationSource: projectPath(appSource),
    importSpecifier: "vue",
    staticExportNames: selected.exportNames,
    selectedArtifact: fileEvidence(selectedCandidateModule),
    reusableArtifact: fileEvidence(candidateModule),
    lilscriptSource: fileEvidence(reactivitySource),
    selectedLilscriptSource: fileEvidence(selected.sourcePath),
    selectedSourceKind: selected.sourceKind,
    productionConfig: fileEvidence(productionConfig),
    hostAdapter: {
      ...fileEvidence(hostAdapter),
      accounting: "inlined before Vite; every retained adapter byte is part of the measured entry chunk",
    },
    parser: {
      name: lexerPackage.name,
      version: lexerPackage.version,
      packageJson: fileEvidence(lexerPackagePath),
    },
  };
  const upstream = await buildVariant("upstream", integration);
  const reusableCandidate = await buildVariant("reusable-candidate", integration);
  const candidate = await buildVariant("candidate", integration, true);
  if (
    upstream.execution.result !== candidate.execution.result ||
    upstream.execution.result !== reusableCandidate.execution.result
  ) {
    throw new Error("paired bundles returned different deterministic results");
  }

  const report = {
    schemaVersion: 1,
    generatedBy: "scripts/build-project-comparison.mjs",
    scenario: {
      id: scenarioId,
      completionRequired: false,
      description: scenario.description,
    },
    upstream: {
      package: "vue",
      version: scope.upstream.version,
      revision: scope.upstream.revision,
      npmIntegrity: lock.packages?.["node_modules/vue"]?.integrity ?? null,
      packageJson: fileEvidence(vuePackagePath),
    },
    input: inputEvidence(),
    toolchain: {
      node: process.version,
      vite: { version: vitePackage.version, packageJson: fileEvidence(vitePackagePath) },
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
    variants: { upstream, candidate },
    diagnostics: {
      reusableCandidate: {
        purpose: "retained-code baseline before source-derived closed-world selection",
        ...reusableCandidate,
      },
    },
    comparison: {
      sameInput: true,
      sameBuildConfig: true,
      onlyModuleResolutionChanged: true,
      onlyResolutionAndBuildIntegrationChanged: true,
      deterministicChecksum: upstream.execution.checksum,
      matchingExecution: true,
      candidateNoUpstreamRuntime: candidate.audit.noUpstreamRuntime,
      passed: true,
    },
  };
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    `Built paired ${scenarioId} bundles with matching checksum ${report.comparison.deterministicChecksum}.`,
  );
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === import.meta.filename) {
  await buildProjectComparison();
}
