import { dirname, resolve } from "node:path";
import { defineConfig } from "vitest/config";

const projectRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(projectRoot, "upstream/vue");
const reactivityRoot = resolve(upstreamRoot, "packages/reactivity/src");
const candidate = resolve(projectRoot, "tests/reactivity-upstream.candidate.js");

const candidateModules = new Set([
  reactivityRoot,
  resolve(reactivityRoot, "index"),
  resolve(reactivityRoot, "index.ts"),
  resolve(reactivityRoot, "arrayInstrumentations"),
  resolve(reactivityRoot, "arrayInstrumentations.ts"),
  resolve(reactivityRoot, "computed"),
  resolve(reactivityRoot, "computed.ts"),
  resolve(reactivityRoot, "constants"),
  resolve(reactivityRoot, "constants.ts"),
  resolve(reactivityRoot, "effect"),
  resolve(reactivityRoot, "effect.ts"),
  resolve(reactivityRoot, "effectScope"),
  resolve(reactivityRoot, "effectScope.ts"),
  resolve(reactivityRoot, "dep"),
  resolve(reactivityRoot, "dep.ts"),
  resolve(reactivityRoot, "reactive"),
  resolve(reactivityRoot, "reactive.ts"),
  resolve(reactivityRoot, "ref"),
  resolve(reactivityRoot, "ref.ts"),
  resolve(reactivityRoot, "watch"),
  resolve(reactivityRoot, "watch.ts"),
]);

export default defineConfig({
  define: {
    __DEV__: true,
    __TEST__: true,
    __VERSION__: '"3.5.42"',
    __BROWSER__: false,
    __GLOBAL__: false,
    __ESM_BUNDLER__: true,
    __ESM_BROWSER__: false,
    __CJS__: false,
    __SSR__: false,
    __FEATURE_OPTIONS_API__: true,
    __FEATURE_SUSPENSE__: true,
    __FEATURE_PROD_DEVTOOLS__: false,
    __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
    __COMPAT__: false,
  },
  plugins: [
    {
      name: "vuelil-reactivity-candidate",
      enforce: "pre",
      resolveId(source, importer) {
        if (source === "@vue/reactivity") return candidate;
        if (!importer || !importer.includes("packages/reactivity/__tests__")) return null;
        if (!source.startsWith(".")) return null;
        const resolved = resolve(dirname(importer), source);
        return candidateModules.has(resolved) ? candidate : null;
      },
    },
  ],
  resolve: {
    alias: {
      "@vue/runtime-core": resolve(upstreamRoot, "packages/runtime-core/src/index.ts"),
      "@vue/runtime-dom": resolve(upstreamRoot, "packages/runtime-dom/src/index.ts"),
      "@vue/runtime-test": resolve(upstreamRoot, "packages/runtime-test/src/index.ts"),
      "@vue/shared": resolve(upstreamRoot, "packages/shared/src/index.ts"),
    },
  },
  test: {
    globals: true,
    root: upstreamRoot,
    setupFiles: resolve(upstreamRoot, "scripts/setup-vitest.ts"),
    pool: "forks",
    execArgv: ["--expose-gc"],
    sequence: { hooks: "list" },
  },
});
