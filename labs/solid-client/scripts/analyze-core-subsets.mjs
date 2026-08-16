import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import { root } from "./project.mjs";

const signalExports = [
  "batch",
  "createComputed",
  "createEffect",
  "createMemo",
  "createRenderEffect",
  "createRoot",
  "createSignal",
  "getListener",
  "getOwner",
  "onCleanup",
  "onMount",
  "runWithOwner",
  "untrack",
];
const webRetainedCore = [
  "ErrorBoundary",
  "For",
  "Index",
  "Match",
  "Show",
  "Suspense",
  "SuspenseList",
  "Switch",
  "createComponent",
  "createEffect",
  "createMemo",
  "createRenderEffect",
  "createRoot",
  "createSignal",
  "enableHydration",
  "getOwner",
  "mergeProps",
  "onCleanup",
  "runWithOwner",
  "sharedConfig",
  "splitProps",
  "untrack",
];
const groups = {
  "web-retained-core": webRetainedCore,
  signals: signalExports,
  errors: ["ErrorBoundary", "catchError", "onError", "resetErrorBoundaries"],
  props: ["createComponent", "mergeProps", "splitProps"],
  arrays: ["For", "Index", "indexArray", "mapArray"],
  control: ["Match", "Show", "Switch", "children"],
  suspense: ["Suspense", "SuspenseList"],
  resource: ["createResource", "lazy", "startTransition", "useTransition"],
  secondary: [
    "createContext",
    "createDeferred",
    "createReaction",
    "createSelector",
    "createUniqueId",
    "enableExternalSource",
    "enableHydration",
    "enableScheduling",
    "from",
    "observable",
    "on",
    "requestCallback",
    "sharedConfig",
    "useContext",
  ],
};
if (process.argv.includes("--individual"))
  for (const name of signalExports) groups[`signal:${name}`] = [name];
const sources = {
  solid: resolve(root, "node_modules/solid-js/dist/solid.js"),
  solidlil: resolve(root, "packages/solidlil/index.js"),
};
const temporary = mkdtempSync(join(tmpdir(), "solidlil-attribution-"));

function size(code) {
  const measured = canonicalCodecSizes(code, "SolidLil core subset analysis");
  return {
    raw: measured.raw,
    gzip9: measured.gzip,
    brotli11: measured.brotli,
  };
}

async function bundle(entry) {
  const result = await build({
    configFile: false,
    root,
    logLevel: "error",
    resolve: { conditions: ["browser", "module", "import", "default"] },
    build: {
      target: "es2022",
      minify: "oxc",
      write: false,
      lib: { entry, formats: ["es"], fileName: "bundle" },
      rolldownOptions: { output: { codeSplitting: false } },
    },
  });
  const outputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunk = outputs.find((item) => item.type === "chunk");
  return size(`${chunk.code.trim()}\n`);
}

try {
  if (process.argv.includes("--web-marginal")) {
    const full = {};
    for (const [implementation, source] of Object.entries(sources)) {
      const entry = resolve(temporary, `web-full-${implementation}.mjs`);
      writeFileSync(
        entry,
        `export { ${webRetainedCore.join(", ")} } from ${JSON.stringify(source)};\n`,
      );
      full[implementation] = await bundle(entry);
    }
    const marginal = [];
    for (const removed of webRetainedCore) {
      const names = webRetainedCore.filter((name) => name !== removed);
      const without = {};
      for (const [implementation, source] of Object.entries(sources)) {
        const entry = resolve(
          temporary,
          `web-without-${removed}-${implementation}.mjs`,
        );
        writeFileSync(
          entry,
          `export { ${names.join(", ")} } from ${JSON.stringify(source)};\n`,
        );
        without[implementation] = await bundle(entry);
      }
      const solid = {
        brotli11: full.solid.brotli11 - without.solid.brotli11,
        gzip9: full.solid.gzip9 - without.solid.gzip9,
        raw: full.solid.raw - without.solid.raw,
      };
      const solidlil = {
        brotli11: full.solidlil.brotli11 - without.solidlil.brotli11,
        gzip9: full.solidlil.gzip9 - without.solidlil.gzip9,
        raw: full.solidlil.raw - without.solidlil.raw,
      };
      marginal.push({
        export: removed,
        solid,
        solidlil,
        gap: {
          brotli11: solidlil.brotli11 - solid.brotli11,
          gzip9: solidlil.gzip9 - solid.gzip9,
          raw: solidlil.raw - solid.raw,
        },
      });
    }
    marginal.sort((left, right) => right.gap.gzip9 - left.gap.gzip9);
    console.log(
      JSON.stringify(
        {
          codecs: canonicalCodecProvenance("SolidLil core subset analysis"),
          full,
          marginal,
        },
        null,
        2,
      ),
    );
    process.exitCode = 0;
  } else {
    const report = {
      codecs: canonicalCodecProvenance("SolidLil core subset analysis"),
    };
    for (const [group, names] of Object.entries(groups)) {
      report[group] = {};
      for (const [name, source] of Object.entries(sources)) {
        const entry = resolve(temporary, `${group}-${name}.mjs`);
        writeFileSync(
          entry,
          `export { ${names.join(", ")} } from ${JSON.stringify(source)};\n`,
        );
        report[group][name] = await bundle(entry);
      }
      report[group].brotliRatio =
        report[group].solidlil.brotli11 / report[group].solid.brotli11;
    }
    console.log(JSON.stringify(report, null, 2));
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
