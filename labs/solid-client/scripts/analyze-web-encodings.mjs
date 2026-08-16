import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build } from "vite";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import { root } from "./project.mjs";

const sourcePath = resolve(root, "packages/solidlil/web.js");
const corePath = resolve(root, "packages/solidlil/index.js");
const clientEntryPath = resolve(root, "api/solidlil-web-client.js");
const clientOnly = process.argv.includes("--client");
const temporary = mkdtempSync(join(tmpdir(), "solidlil-web-encodings-"));
const source = readFileSync(sourcePath, "utf8").replaceAll(
  '"./index.js"',
  JSON.stringify(corePath),
);
const splitString = /"([^"\n]+)"\.split\(\s*" "\s*,?\s*\)/g;

function tableName(value) {
  if (value.startsWith("className ")) return "properties";
  if (value.startsWith("innerHTML ")) return "children";
  if (value.startsWith("beforeinput ")) return "events";
  if (value.startsWith("altGlyph ")) return "svg";
  if (value.startsWith("html ")) return "dom";
  return null;
}

function delimit(code, delimiter, selected) {
  return code.replace(splitString, (match, value) => {
    const name = tableName(value);
    if (!name || (selected && name !== selected)) return match;
    return `${JSON.stringify(value.replaceAll(" ", delimiter))}.split(${JSON.stringify(delimiter)})`;
  });
}

function arrays(code, selected) {
  return code.replace(splitString, (match, value) => {
    const name = tableName(value);
    if (!name || (selected && name !== selected)) return match;
    return JSON.stringify(value.split(" "));
  });
}

function size(code) {
  const measured = canonicalCodecSizes(code, "SolidLil Web encoding analysis");
  return {
    brotli11: measured.brotli,
    gzip9: measured.gzip,
    raw: measured.raw,
  };
}

async function bundle(name, code) {
  const implementation = resolve(temporary, `${name}.mjs`);
  const entry = resolve(temporary, `${name}-entry.mjs`);
  writeFileSync(implementation, code);
  writeFileSync(
    entry,
    clientOnly
      ? readFileSync(clientEntryPath, "utf8").replace(
          '"../packages/solidlil/web.js"',
          JSON.stringify(implementation),
        )
      : `export * from ${JSON.stringify(implementation)};\n`,
  );
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
  const variants = { current: source };
  variants["all-array"] = arrays(source);
  for (const name of ["properties", "children", "events", "svg", "dom"])
    variants[`${name}-array`] = arrays(source, name);
  for (const delimiter of [".", ",", "|"]) {
    variants[`all-${delimiter.charCodeAt(0)}`] = delimit(source, delimiter);
    for (const name of ["properties", "children", "events", "svg", "dom"])
      variants[`${name}-${delimiter.charCodeAt(0)}`] = delimit(
        source,
        delimiter,
        name,
      );
  }
  const report = {
    codecs: canonicalCodecProvenance("SolidLil Web encoding analysis"),
  };
  for (const [name, code] of Object.entries(variants))
    report[name] = await bundle(name, code);
  console.log(JSON.stringify(report, null, 2));
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
