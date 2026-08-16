import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "../upstream/solid/node_modules/vitest/dist/config.js";
import solidPlugin from "../upstream/solid/node_modules/vite-plugin-solid/dist/esm/index.mjs";

const labRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packageRoot = resolve(labRoot, "upstream/solid/packages/solid");
const candidate = {
  core: resolve(labRoot, "packages/solidlil/index.js"),
  store: resolve(labRoot, "packages/solidlil/store.js"),
  web: resolve(labRoot, "packages/solidlil/web.js"),
};
const relativeEntries = new Map([
  [resolve(packageRoot, "src/index.js"), candidate.core],
  [resolve(packageRoot, "store/src/index.js"), candidate.store],
  [resolve(packageRoot, "web/src/index.js"), candidate.web],
]);

function candidateResolver() {
  return {
    name: "solidlil-upstream-candidate",
    enforce: "pre",
    resolveId(source, importer) {
      if (source === "solid-js") return candidate.core;
      if (source === "solid-js/store") return candidate.store;
      if (source === "solid-js/web" || source === "solid-js/jsx-runtime")
        return candidate.web;
      if (source === "rxcore") return resolve(packageRoot, "web/src/core.ts");
      if (!importer || !source.startsWith(".")) return null;
      const importerPath = importer.split("?", 1)[0];
      return (
        relativeEntries.get(resolve(dirname(importerPath), source)) ?? null
      );
    },
  };
}

export default defineConfig({
  root: packageRoot,
  plugins: [candidateResolver(), solidPlugin()],
  test: {
    environment: "jsdom",
    transformMode: { web: [/\.[jt]sx?$/] },
    deps: { registerNodeLoader: true },
    globals: true,
    // The pinned files intentionally mutate process-global scheduler, timer,
    // and DEV-hook state. Keep their source unchanged, but give each file the
    // same clean module boundary it receives in Solid's own test runs.
    isolate: true,
    threads: false,
  },
  resolve: {
    conditions: ["development", "browser"],
  },
});
