import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import { root } from "./project.mjs";

const temporary = mkdtempSync(join(tmpdir(), "solidlil-web-layers-"));

function size(code) {
  const measured = canonicalCodecSizes(code, "SolidLil web-layer analysis");
  return {
    brotli11: measured.brotli,
    gzip9: measured.gzip,
    raw: measured.raw,
  };
}

async function bundle(entry, external) {
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
      rolldownOptions: {
        external,
        output: { codeSplitting: false },
      },
    },
  });
  const outputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunk = outputs.find((item) => item.type === "chunk");
  return size(`${chunk.code.trim()}\n`);
}

try {
  const solidEntry = resolve(root, "node_modules/solid-js/web/dist/web.js");
  const solidlilCore = resolve(root, "packages/solidlil/index.js");
  const solidlilSource = readFileSync(
    resolve(root, "packages/solidlil/web.js"),
    "utf8",
  ).replaceAll('"./index.js"', JSON.stringify(solidlilCore));
  const solidlilEntry = resolve(temporary, "solidlil-web.mjs");
  writeFileSync(solidlilEntry, solidlilSource);
  const report = {
    codecs: canonicalCodecProvenance("SolidLil web-layer analysis"),
    solid: await bundle(solidEntry, (id) => id === "solid-js"),
    solidlil: await bundle(solidlilEntry, (id) => id === solidlilCore),
  };
  console.log(JSON.stringify(report, null, 2));
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
