import { writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { defineConfig } from "vitest/config";
import { entries as vueSourceEntries } from "../upstream/vue/scripts/aliases.js";
import { runtimeCoreTestFlags } from "./runtime-core-test-flags.mjs";

const labRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(labRoot, "upstream/vue");
const runtimeCoreRoot = resolve(upstreamRoot, "packages/runtime-core/src");
const candidate = resolve(labRoot, "tests/runtime-core-upstream.candidate.mjs");
const reactivityCandidate = resolve(labRoot, "packages/vuelil/reactivity.js");
const sharedCandidate = resolve(labRoot, "packages/vuelil/shared.js");
const auditPath = process.env.VUELIL_RUNTIME_CORE_AUDIT;
const redirects = new Set();
const blocked = new Set();
let candidateLoaded = false;

function clean(id) {
  return id.split(/[?#]/, 1)[0].replace(/^\/@fs\//, "/");
}

function inside(path, root) {
  const child = relative(root, path);
  return child === "" || (child !== ".." && !child.startsWith(`..${sep}`));
}

function targetFor(source, importer) {
  if (source === "@vue/runtime-core" || source.startsWith("@vue/runtime-core/")) return true;
  const sourcePath = clean(source);
  if (isAbsolute(sourcePath) && inside(sourcePath, runtimeCoreRoot)) return true;
  if (importer && source.startsWith(".")) {
    return inside(resolve(dirname(clean(importer)), source), runtimeCoreRoot);
  }
  return false;
}

function writeAudit() {
  if (!auditPath) return;
  writeFileSync(auditPath, `${JSON.stringify({
    candidateLoaded,
    redirects: [...redirects].sort(),
    blocked: [...blocked].sort(),
  }, null, 2)}\n`);
}

const aliases = Object.fromEntries(
  Object.entries(vueSourceEntries).filter(([name]) => name !== "@vue/runtime-core"),
);
aliases["@vue/reactivity"] = reactivityCandidate;
aliases["@vue/shared"] = sharedCandidate;

export default defineConfig({
  root: upstreamRoot,
  plugins: [{
    name: "vuelil-runtime-core-candidate",
    enforce: "pre",
    resolveId(source, importer) {
      if (source === "@vue/reactivity") return reactivityCandidate;
      if (source === "@vue/shared") return sharedCandidate;
      if (targetFor(source, importer)) {
        redirects.add(`${source} <- ${importer ?? "entry"}`);
        writeAudit();
        return candidate;
      }
      return null;
    },
    load(id) {
      const path = clean(id);
      if (path === candidate) {
        candidateLoaded = true;
        writeAudit();
      } else if (isAbsolute(path) && inside(path, runtimeCoreRoot)) {
        blocked.add(path);
        writeAudit();
        throw new Error(`blocked upstream runtime-core implementation: ${path}`);
      }
      return null;
    },
    closeBundle: writeAudit,
  }],
  define: Object.fromEntries(
    Object.entries(runtimeCoreTestFlags).map(([name, value]) => [name, JSON.stringify(value)]),
  ),
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
