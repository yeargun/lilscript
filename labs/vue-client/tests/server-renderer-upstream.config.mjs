import { writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const roots = {
  serverRenderer: resolve(upstreamRoot, "packages/server-renderer/src"),
  runtimeCore: resolve(upstreamRoot, "packages/runtime-core/src"),
  runtimeDom: resolve(upstreamRoot, "packages/runtime-dom/src"),
  runtimeTest: resolve(upstreamRoot, "packages/runtime-test/src"),
  vue: resolve(upstreamRoot, "packages/vue/src"),
};
const candidates = {
  serverRenderer: resolve(labRoot, "tests/server-renderer-upstream.candidate.mjs"),
  runtimeCore: resolve(labRoot, "packages/vuelil/runtime-core.internal.js"),
  runtimeDom: resolve(labRoot, "packages/vuelil/runtime-dom.js"),
  runtimeTest: resolve(labRoot, "packages/vuelil/runtime-test.js"),
  vue: resolve(labRoot, "packages/vuelil/runtime-dom.js"),
};
const auditPath = process.env.VUELIL_SERVER_RENDERER_AUDIT;
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

function targets(source, importer, packageNames, sourceRoot) {
  if (packageNames.some(name => source === name || source.startsWith(`${name}/`))) return true;
  const requested = source.startsWith(".") && importer
    ? resolve(dirname(clean(importer)), source)
    : clean(source);
  return isAbsolute(requested) && inside(requested, sourceRoot);
}

function writeAudit() {
  if (!auditPath) return;
  writeFileSync(auditPath, `${JSON.stringify({
    loaded: [...loaded].sort(),
    redirects: [...redirects].sort(),
    blocked: [...blocked].sort(),
    runtimeTestAlias: candidates.runtimeTest,
  }, null, 2)}\n`);
}

const aliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(([name]) => ![
    "vue",
    "vue/server-renderer",
    "@vue/server-renderer",
    "@vue/runtime-core",
    "@vue/runtime-dom",
    "@vue/runtime-test",
  ].includes(name)),
);
aliases["@vue/reactivity"] = resolve(labRoot, "packages/vuelil/reactivity.js");
aliases["@vue/shared"] = resolve(labRoot, "packages/vuelil/shared.js");

const targetsByName = [
  ["serverRenderer", ["@vue/server-renderer", "vue/server-renderer"]],
  ["runtimeTest", ["@vue/runtime-test"]],
  ["runtimeDom", ["@vue/runtime-dom"]],
  ["runtimeCore", ["@vue/runtime-core"]],
  ["vue", ["vue"]],
];

export default defineConfig({
  root: upstreamRoot,
  plugins: [{
    name: "vuelil-server-renderer-candidate",
    enforce: "pre",
    resolveId(source, importer) {
      for (const [name, packages] of targetsByName) {
        if (targets(source, importer, packages, roots[name])) {
          redirects.add(`${name}: ${source} <- ${importer ?? "entry"}`);
          writeAudit();
          return candidates[name];
        }
      }
      return null;
    },
    load(id) {
      const path = clean(id);
      for (const [name, candidate] of Object.entries(candidates)) {
        if (path === candidate) loaded.add(name);
      }
      for (const [name, root] of Object.entries(roots)) {
        if (isAbsolute(path) && inside(path, root)) {
          blocked.add(`${name}: ${path}`);
          writeAudit();
          throw new Error(`blocked upstream ${name} implementation: ${path}`);
        }
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
    setupFiles: [
      resolve(labRoot, "tests/server-renderer-upstream.setup.mjs"),
      resolve(upstreamRoot, "scripts/setup-vitest.ts"),
    ],
    sequence: { hooks: "list" },
  },
});
