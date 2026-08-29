import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import {
  dirname,
  isAbsolute,
  relative,
  resolve,
  sep,
} from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

export const labRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const upstreamRoot = resolve(labRoot, "upstream/vue");
export const testMapPath = resolve(labRoot, "compatibility/test-map.json");

function cleanModuleId(id) {
  if (typeof id !== "string") return "";
  let clean = id.split(/[?#]/, 1)[0];
  if (clean.startsWith("/@fs/")) clean = clean.slice(4);
  try {
    clean = decodeURIComponent(clean);
  } catch {
    // Vite IDs are normally URI encoded, but a literal percent is still valid.
  }
  return clean;
}

function isInside(path, root) {
  const child = relative(root, path);
  return child === "" || (!child.startsWith(`..${sep}`) && child !== "..");
}

function toPosix(path) {
  return path.split(sep).join("/");
}

function atomicWriteJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  renameSync(temporary, path);
}

export function loadTestMap() {
  const map = JSON.parse(readFileSync(testMapPath, "utf8"));
  if (map.schemaVersion !== 1) {
    throw new Error(`Unsupported Vue test-map schema ${map.schemaVersion}`);
  }
  return map;
}

export function resolveTestSuite(name, map = loadTestMap()) {
  const suite = map.suites?.[name];
  if (!suite) throw new Error(`Unknown Vue upstream test suite: ${name}`);
  if (!Array.isArray(suite.targetPackages) || suite.targetPackages.length === 0) {
    throw new Error(`Suite ${name} has no candidate target packages`);
  }
  const targets = suite.targetPackages.map((packageName) => {
    const definition = map.packages?.[packageName];
    if (!definition) throw new Error(`Suite ${name} references unknown ${packageName}`);
    if (!definition.candidateFacade) {
      throw new Error(`Suite ${name} has no candidate facade for ${packageName}`);
    }
    return {
      package: packageName,
      sourceRoot: resolve(upstreamRoot, definition.sourceRoot),
      candidateFacade: resolve(labRoot, definition.candidateFacade),
    };
  });
  return { name, ...suite, targets };
}

export function createVueTestAliases({
  suiteName = "shared",
  map = loadTestMap(),
  auditPath = null,
} = {}) {
  const suite = resolveTestSuite(suiteName, map);
  const redirects = new Map();
  const candidateLoads = new Set();
  const upstreamLoads = new Map();
  const blockedLoads = new Map();
  let configured = false;
  let completed = false;

  function identifyUpstreamModule(id) {
    const path = cleanModuleId(id);
    if (!isAbsolute(path) || !isInside(path, upstreamRoot)) return null;
    const relativePath = toPosix(relative(upstreamRoot, path));
    const match = /^packages\/([^/]+)\/src(?:\/|$)/.exec(relativePath);
    if (!match) return null;
    const packageName = match[1] === "vue" ? "vue" : `@vue/${match[1]}`;
    return { package: packageName, path: relativePath };
  }

  function targetForPath(id) {
    const path = cleanModuleId(id);
    if (!isAbsolute(path)) return null;
    return suite.targets.find((target) => isInside(path, target.sourceRoot)) ?? null;
  }

  function snapshot() {
    const targets = suite.targets.map((target) => ({
      package: target.package,
      sourceRoot: toPosix(relative(upstreamRoot, target.sourceRoot)),
      candidateFacade: toPosix(relative(labRoot, target.candidateFacade)),
      redirects: [...redirects.values()].filter(
        (entry) => entry.package === target.package,
      ).length,
      candidateLoaded: candidateLoads.has(target.package),
    }));
    const blockedModules = [...blockedLoads.values()];
    return {
      schemaVersion: 1,
      suite: suite.name,
      completed,
      targets,
      redirects: [...redirects.values()],
      upstreamImplementationModules: [...upstreamLoads.values()],
      blockedTargetedModules: blockedModules,
      noTargetedUpstreamImplementationImports:
        completed && blockedModules.length === 0,
    };
  }

  function flush() {
    if (auditPath) atomicWriteJson(auditPath, snapshot());
  }

  function finish() {
    if (completed) return;
    completed = true;
    flush();
    process.removeListener("beforeExit", finish);
  }

  function recordRedirect(source, importer, target) {
    const importerPath = cleanModuleId(importer);
    const entry = {
      package: target.package,
      specifier: source,
      importer:
        isAbsolute(importerPath) && isInside(importerPath, labRoot)
          ? toPosix(relative(labRoot, importerPath))
          : importerPath || null,
      candidateFacade: toPosix(relative(labRoot, target.candidateFacade)),
    };
    redirects.set(JSON.stringify(entry), entry);
    flush();
  }

  function inspectLoad(id, hook) {
    const path = cleanModuleId(id);
    for (const target of suite.targets) {
      if (path === target.candidateFacade) {
        candidateLoads.add(target.package);
        flush();
      }
    }
    const upstreamModule = identifyUpstreamModule(path);
    if (!upstreamModule) return;
    const target = targetForPath(path);
    if (target) {
      const blocked = { ...upstreamModule, hook };
      blockedLoads.set(`${blocked.path}:${hook}`, blocked);
      flush();
      throw new Error(
        `Blocked targeted upstream implementation module ${blocked.path}; ${target.package} must resolve to ${toPosix(relative(labRoot, target.candidateFacade))}`,
      );
    }
    upstreamLoads.set(upstreamModule.path, upstreamModule);
    flush();
  }

  return {
    name: "vuelil-upstream-candidate-aliases",
    enforce: "pre",
    configResolved() {
      if (configured) return;
      configured = true;
      completed = false;
      flush();
      process.once("beforeExit", finish);
    },
    resolveId(source, importer) {
      for (const target of suite.targets) {
        if (source === target.package || source.startsWith(`${target.package}/`)) {
          recordRedirect(source, importer, target);
          return target.candidateFacade;
        }
      }
      const sourcePath = cleanModuleId(source);
      if (isAbsolute(sourcePath)) {
        const target = targetForPath(sourcePath);
        if (target) {
          recordRedirect(source, importer, target);
          return target.candidateFacade;
        }
      }
      const importerPath = cleanModuleId(importer);
      if (importerPath && source.startsWith(".")) {
        const requestedPath = resolve(dirname(importerPath), source);
        const target = targetForPath(requestedPath);
        if (target) {
          recordRedirect(source, importer, target);
          return target.candidateFacade;
        }
      }
      return null;
    },
    load(id) {
      inspectLoad(id, "load");
      return null;
    },
    transform(_code, id) {
      inspectLoad(id, "transform");
      return null;
    },
    closeBundle: finish,
    api: { snapshot, finish },
  };
}

const configuredSuiteName = process.env.VUELIL_TEST_SUITE ?? "shared";
const configuredMap = loadTestMap();
const configuredSuite = resolveTestSuite(configuredSuiteName, configuredMap);
const configuredTargets = new Set(configuredSuite.targetPackages);
const configuredAuditPath =
  process.env.VUELIL_AUDIT_PATH ??
  resolve(tmpdir(), `vuelil-upstream-audit-${process.pid}.json`);
const nonTargetAliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(([name]) => !configuredTargets.has(name)),
);

export default defineConfig({
  root: upstreamRoot,
  plugins: [
    createVueTestAliases({
      suiteName: configuredSuiteName,
      map: configuredMap,
      auditPath: configuredAuditPath,
    }),
  ],
  define: {
    __DEV__: true,
    __TEST__: true,
    __VERSION__: '"test"',
    __BROWSER__: false,
    __GLOBAL__: false,
    __ESM_BUNDLER__: true,
    __ESM_BROWSER__: false,
    __CJS__: true,
    __SSR__: true,
    __FEATURE_OPTIONS_API__: true,
    __FEATURE_SUSPENSE__: true,
    __FEATURE_PROD_DEVTOOLS__: false,
    __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
    __COMPAT__: true,
  },
  resolve: {
    alias: nonTargetAliases,
  },
  server: {
    fs: { allow: [labRoot, upstreamRoot] },
  },
  test: {
    globals: true,
    isolate: true,
    pool: "threads",
    setupFiles: resolve(upstreamRoot, "scripts/setup-vitest.ts"),
    sequence: { hooks: "list" },
  },
});
