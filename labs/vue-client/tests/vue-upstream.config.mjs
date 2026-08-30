import { writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { playwright } from "../upstream/vue/node_modules/@vitest/browser-playwright/dist/index.js";
import { defineConfig } from "vitest/config";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const browserMode = process.env.VUELIL_VUE_BROWSER === "1";
const auditPath = process.env.VUELIL_VUE_AUDIT;
const targets = [
  ["vue", ["vue"], "packages/vue/src", "tests/vue-upstream.candidate.mjs"],
  ["@vue/compat", ["@vue/compat"], "packages/vue-compat/src", "packages/vuelil/vue-compat.js"],
  ["@vue/compiler-core", ["@vue/compiler-core"], "packages/compiler-core/src", "packages/vuelil/compiler-core.js"],
  ["@vue/compiler-dom", ["@vue/compiler-dom"], "packages/compiler-dom/src", "tests/compiler-dom-upstream.candidate.mjs"],
  ["@vue/compiler-sfc", ["@vue/compiler-sfc", "vue/compiler-sfc"], "packages/compiler-sfc/src", "packages/vuelil/compiler-sfc.js"],
  ["@vue/compiler-ssr", ["@vue/compiler-ssr"], "packages/compiler-ssr/src", "packages/vuelil/compiler-ssr.js"],
  ["@vue/reactivity", ["@vue/reactivity"], "packages/reactivity/src", "packages/vuelil/reactivity.js"],
  ["@vue/runtime-core", ["@vue/runtime-core"], "packages/runtime-core/src", "tests/runtime-core-upstream.candidate.mjs"],
  ["@vue/runtime-dom", ["@vue/runtime-dom"], "packages/runtime-dom/src", "tests/runtime-dom-upstream.candidate.mjs"],
  ["@vue/runtime-test", ["@vue/runtime-test"], "packages/runtime-test/src", "packages/vuelil/runtime-test.js"],
  ["@vue/server-renderer", ["@vue/server-renderer", "vue/server-renderer"], "packages/server-renderer/src", "packages/vuelil/server-renderer.js"],
  ["@vue/shared", ["@vue/shared"], "packages/shared/src", "packages/vuelil/shared.js"],
].map(([packageName, specifiers, source, candidate]) => ({
  packageName,
  specifiers,
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
    if (target.specifiers.some(specifier =>
      source === specifier ||
      (specifier.startsWith("@") && source.startsWith(`${specifier}/`))
    )) return target;
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
    browserMode,
    loadedCandidates: [...loaded].sort(),
    redirects: [...redirects].sort(),
    blockedUpstreamImplementations: [...blocked].sort(),
  }, null, 2)}\n`);
}

export default defineConfig({
  root: upstreamRoot,
  plugins: [{
    name: "vuelil-vue-candidate",
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
    __BROWSER__: browserMode,
    __GLOBAL__: false,
    __ESM_BUNDLER__: true,
    __ESM_BROWSER__: false,
    __CJS__: true,
    __SSR__: true,
    __FEATURE_OPTIONS_API__: true,
    __FEATURE_SUSPENSE__: true,
    __FEATURE_PROD_DEVTOOLS__: false,
    __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
    __COMPAT__: false,
  },
  server: { fs: { allow: [labRoot, upstreamRoot] } },
  test: {
    globals: true,
    isolate: true,
    pool: "threads",
    setupFiles: resolve(upstreamRoot, "scripts/setup-vitest.ts"),
    sequence: { hooks: "list" },
    ...(browserMode
      ? {
          browser: {
            enabled: true,
            provider: playwright({
              launchOptions: {
                executablePath: process.env.VUELIL_CHROMIUM_EXECUTABLE,
              },
            }),
            headless: true,
            instances: [{ browser: "chromium" }],
          },
        }
      : { environment: "jsdom" }),
  },
});
