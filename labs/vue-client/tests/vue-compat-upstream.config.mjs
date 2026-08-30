import { writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const auditPath = process.env.VUELIL_VUE_COMPAT_AUDIT;
const targets = [
  ["@vue/compat", "packages/vue-compat/src", "tests/vue-compat-upstream.candidate.mjs"],
  ["@vue/runtime-dom", "packages/runtime-dom/src", "tests/vue-compat-runtime-dom.candidate.mjs"],
  ["@vue/runtime-core", "packages/runtime-core/src", "tests/runtime-core-upstream.candidate.mjs"],
  ["@vue/compiler-dom", "packages/compiler-dom/src", "tests/compiler-dom-upstream.candidate.mjs"],
  ["@vue/compiler-core", "packages/compiler-core/src", "packages/vuelil/compiler-core.js"],
  ["@vue/reactivity", "packages/reactivity/src", "packages/vuelil/reactivity.js"],
  ["@vue/shared", "packages/shared/src", "packages/vuelil/shared.js"],
].map(([packageName, source, candidate]) => ({
  packageName,
  source: resolve(upstreamRoot, source),
  candidate: resolve(labRoot, candidate),
}));
const redirects = new Set();
const loaded = new Set();
const blocked = new Set();

function clean(id) {
  return id.split(/[?#]/u, 1)[0].replace(/^\/@fs\//u, "/");
}

function inside(path, root) {
  const child = relative(root, path);
  return child === "" || (child !== ".." && !child.startsWith(`..${sep}`));
}

function targetFor(source, importer) {
  for (const target of targets) {
    if (source === target.packageName || source.startsWith(`${target.packageName}/`)) {
      return target;
    }
  }
  const requested = source.startsWith(".") && importer
    ? resolve(dirname(clean(importer)), source)
    : clean(source);
  if (!isAbsolute(requested)) return null;
  return targets.find(target => inside(requested, target.source)) ?? null;
}

function writeAudit() {
  if (!auditPath) return;
  writeFileSync(auditPath, `${JSON.stringify({
    loadedCandidates: [...loaded].sort(),
    redirects: [...redirects].sort(),
    blockedUpstreamImplementations: [...blocked].sort(),
  }, null, 2)}\n`);
}

const targetPackages = new Set(targets.map(target => target.packageName));
const aliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(([name]) => !targetPackages.has(name)),
);

export default defineConfig({
  root: upstreamRoot,
  plugins: [{
    name: "vuelil-vue-compat-candidate",
    enforce: "pre",
    configResolved() {
      process.once("beforeExit", writeAudit);
      writeAudit();
    },
    resolveId(source, importer) {
      const target = targetFor(source, importer);
      if (!target) return null;
      redirects.add(`${target.packageName}: ${source} <- ${importer ?? "entry"}`);
      writeAudit();
      return target.candidate;
    },
    load(id) {
      const path = clean(id);
      const candidate = targets.find(target => path === target.candidate);
      if (candidate) {
        loaded.add(candidate.packageName);
        writeAudit();
        return null;
      }
      const upstream = targets.find(target => isAbsolute(path) && inside(path, target.source));
      if (upstream) {
        const entry = `${upstream.packageName}: ${relative(upstreamRoot, path).replaceAll("\\", "/")}`;
        blocked.add(entry);
        writeAudit();
        throw new Error(`blocked upstream ${upstream.packageName} implementation: ${path}`);
      }
      return null;
    },
    transform(_code, id) {
      const path = clean(id);
      const upstream = targets.find(target => isAbsolute(path) && inside(path, target.source));
      if (!upstream) return null;
      const entry = `${upstream.packageName}: ${relative(upstreamRoot, path).replaceAll("\\", "/")}`;
      blocked.add(entry);
      writeAudit();
      throw new Error(`blocked upstream ${upstream.packageName} implementation: ${path}`);
    },
    closeBundle: writeAudit,
  }],
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
  resolve: { alias: aliases },
  server: { fs: { allow: [labRoot, upstreamRoot] } },
  test: {
    globals: true,
    isolate: true,
    pool: "threads",
    environment: "jsdom",
    setupFiles: resolve(upstreamRoot, "scripts/setup-vitest.ts"),
    sequence: { hooks: "list" },
  },
});
