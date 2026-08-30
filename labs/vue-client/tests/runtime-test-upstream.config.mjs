import { writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const runtimeTestRoot = resolve(upstreamRoot, "packages/runtime-test/src");
const runtimeCoreRoot = resolve(upstreamRoot, "packages/runtime-core/src");
const candidate = resolve(labRoot, "packages/vuelil/runtime-test.js");
const core = resolve(labRoot, "packages/vuelil/runtime-core.internal.js");
const auditPath = process.env.VUELIL_RUNTIME_TEST_AUDIT;
const redirects = new Set();
const blocked = new Set();
let candidateLoaded = false;
let coreLoaded = false;

function clean(id) {
  return id.split(/[?#]/, 1)[0].replace(/^\/@fs\//, "/");
}

function inside(path, root) {
  const child = relative(root, path);
  return child === "" || (child !== ".." && !child.startsWith(`..${sep}`));
}

function targets(source, importer, packageName, sourceRoot) {
  if (source === packageName || source.startsWith(`${packageName}/`)) return true;
  const sourcePath = clean(source);
  if (isAbsolute(sourcePath) && inside(sourcePath, sourceRoot)) return true;
  return Boolean(importer && source.startsWith(".") &&
    inside(resolve(dirname(clean(importer)), source), sourceRoot));
}

function writeAudit() {
  if (!auditPath) return;
  writeFileSync(auditPath, `${JSON.stringify({
    candidateLoaded,
    coreLoaded,
    redirects: [...redirects].sort(),
    blocked: [...blocked].sort(),
  }, null, 2)}\n`);
}

const aliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(([name]) =>
    name !== "@vue/runtime-test" && name !== "@vue/runtime-core"),
);
aliases["@vue/reactivity"] = resolve(labRoot, "packages/vuelil/reactivity.js");
aliases["@vue/shared"] = resolve(labRoot, "packages/vuelil/shared.js");

export default defineConfig({
  root: upstreamRoot,
  plugins: [{
    name: "vuelil-runtime-test-candidate",
    enforce: "pre",
    resolveId(source, importer) {
      if (targets(source, importer, "@vue/runtime-test", runtimeTestRoot)) {
        redirects.add(`${source} <- ${importer ?? "entry"}`);
        writeAudit();
        return candidate;
      }
      if (targets(source, importer, "@vue/runtime-core", runtimeCoreRoot)) return core;
      return null;
    },
    load(id) {
      const path = clean(id);
      if (path === candidate) candidateLoaded = true;
      if (path === core) coreLoaded = true;
      if ((isAbsolute(path) && inside(path, runtimeTestRoot)) ||
          (isAbsolute(path) && inside(path, runtimeCoreRoot))) {
        blocked.add(path);
        writeAudit();
        throw new Error(`blocked upstream runtime implementation: ${path}`);
      }
      writeAudit();
      return null;
    },
    closeBundle: writeAudit,
  }],
  define: {
    __DEV__: true,
    __TEST__: true,
    __VERSION__: JSON.stringify("3.5.42"),
    __BROWSER__: false,
    __GLOBAL__: false,
    __ESM_BUNDLER__: true,
    __ESM_BROWSER__: false,
    __CJS__: true,
    __SSR__: false,
    __FEATURE_OPTIONS_API__: true,
    __FEATURE_SUSPENSE__: true,
    __FEATURE_PROD_DEVTOOLS__: false,
    __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
    __COMPAT__: false,
  },
  resolve: { alias: aliases },
  server: { fs: { allow: [labRoot, upstreamRoot] } },
  test: {
    globals: true,
    isolate: true,
    pool: "threads",
    setupFiles: resolve(upstreamRoot, "scripts/setup-vitest.ts"),
    sequence: { hooks: "list" },
  },
});
