import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const compilerSFCSource = resolve(upstreamRoot, "packages/compiler-sfc/src");
const compilerDOMSource = resolve(upstreamRoot, "packages/compiler-dom/src");
const compilerCoreSource = resolve(upstreamRoot, "packages/compiler-core/src");
const candidate = resolve(labRoot, "tests/compiler-sfc-upstream.candidate.mjs");
const warnCandidate = resolve(labRoot, "tests/compiler-sfc-warn.candidate.mjs");
const compilerDOMCandidate = resolve(
  labRoot,
  "tests/compiler-dom-upstream.candidate.mjs",
);
const compilerCoreCandidate = resolve(labRoot, "packages/vuelil/compiler-core.js");

function clean(id) {
  return id.split(/[?#]/u, 1)[0].replace(/^\/@fs\//u, "/");
}

function inside(path, root) {
  const child = relative(root, path);
  return child === "" || (child !== ".." && !child.startsWith(`..${sep}`));
}

function targets(source, importer, packageName, sourceRoot) {
  if (source === packageName || source.startsWith(`${packageName}/`)) return true;
  const requested = source.startsWith(".") && importer
    ? resolve(dirname(clean(importer)), source)
    : clean(source);
  return isAbsolute(requested) && inside(requested, sourceRoot);
}

const aliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(
    ([name]) => ![
      "@vue/compiler-core",
      "@vue/compiler-dom",
      "@vue/compiler-sfc",
    ].includes(name),
  ),
);

export default defineConfig({
  root: upstreamRoot,
  plugins: [{
    name: "vuelil-compiler-sfc-candidate",
    enforce: "pre",
    resolveId(source, importer) {
      const requested = source.startsWith(".") && importer
        ? resolve(dirname(clean(importer)), source)
        : clean(source);
      if (requested === resolve(compilerSFCSource, "warn")) {
        return warnCandidate;
      }
      if (targets(source, importer, "@vue/compiler-sfc", compilerSFCSource)) {
        return candidate;
      }
      if (targets(source, importer, "@vue/compiler-dom", compilerDOMSource)) {
        return compilerDOMCandidate;
      }
      if (targets(source, importer, "@vue/compiler-core", compilerCoreSource)) {
        return compilerCoreCandidate;
      }
      return null;
    },
    load(id) {
      const path = clean(id);
      for (const sourceRoot of [compilerSFCSource, compilerDOMSource, compilerCoreSource]) {
        if (isAbsolute(path) && inside(path, sourceRoot)) {
          throw new Error(`blocked upstream compiler implementation: ${path}`);
        }
      }
      return null;
    },
  }],
  define: {
    __DEV__: true,
    __TEST__: true,
    __VERSION__: '"3.5.42"',
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
  },
});
