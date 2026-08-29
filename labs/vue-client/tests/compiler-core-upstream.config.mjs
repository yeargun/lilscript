import { resolve, dirname } from "node:path";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const compilerSource = resolve(upstreamRoot, "packages/compiler-core/src");
const compilerSfcSource = resolve(upstreamRoot, "packages/compiler-sfc/src");
const candidate = resolve(labRoot, "packages/vuelil/compiler-core.js");
const compilerSfcShim = resolve(labRoot, "tests/compiler-core-sfc-shim.mjs");

function clean(id) {
  return id.split(/[?#]/, 1)[0].replace(/^\/@fs\//, "/");
}

function compilerCoreCandidate() {
  return {
    name: "vuelil-compiler-core-candidate",
    enforce: "pre",
    resolveId(source, importer) {
      if (source === "@vue/compiler-sfc") return compilerSfcShim;
      if (source === "@vue/compiler-core" || source.startsWith("@vue/compiler-core/")) {
        return candidate;
      }
      if (!importer) return null;
      const requested = source.startsWith(".")
        ? resolve(dirname(clean(importer)), source)
        : clean(source);
      if (requested === compilerSfcSource || requested.startsWith(`${compilerSfcSource}/`)) {
        return compilerSfcShim;
      }
      if (requested === compilerSource || requested.startsWith(`${compilerSource}/`)) {
        return candidate;
      }
      return null;
    },
  };
}

const aliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(([name]) => name !== "@vue/compiler-core"),
);

export default defineConfig({
  root: upstreamRoot,
  plugins: [compilerCoreCandidate()],
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
    setupFiles: resolve(upstreamRoot, "scripts/setup-vitest.ts"),
  },
});
