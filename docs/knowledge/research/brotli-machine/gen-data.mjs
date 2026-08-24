#!/usr/bin/env node
/* Extracts the RFC 7932 static tables from the Brotli C sources that this
   repository already vendors through compu-brotli-sys, and writes
   data/tables.js (a plain script, no imports) for the page bundle.

   Run:  node docs/knowledge/research/brotli-machine/gen-data.mjs [--src <brotli/c dir>]

   The generated file is committed, so the page can be rebuilt on a machine
   without the Cargo registry checkout. */
import { readFileSync, writeFileSync, existsSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const here = dirname(fileURLToPath(import.meta.url));

function findBrotliC() {
  const flag = process.argv.indexOf("--src");
  if (flag >= 0) return process.argv[flag + 1];
  const reg = join(process.env.HOME, ".cargo/registry/src");
  if (!existsSync(reg)) throw new Error("no cargo registry; pass --src <brotli/c>");
  for (const index of readdirSync(reg)) {
    for (const crate of readdirSync(join(reg, index))) {
      if (!/^compu-brotli-sys-/.test(crate)) continue;
      const c = join(reg, index, crate, "brotli/c");
      if (existsSync(join(c, "common/dictionary.bin"))) return c;
    }
  }
  throw new Error("compu-brotli-sys brotli/c not found; pass --src <brotli/c>");
}

const C = findBrotliC();
const read = (p) => readFileSync(join(C, p), "utf8");

/* ---- 1. static dictionary ------------------------------------------- */
const dictBytes = readFileSync(join(C, "common/dictionary.bin"));
if (dictBytes.length !== 122784) throw new Error(`dictionary is ${dictBytes.length} bytes`);
const dictSha = createHash("sha256").update(dictBytes).digest("hex");
const dictBr = readFileSync(join(C, "common/dictionary.bin.br"));

const SIZE_BITS = [
  0, 0, 0, 0, 10, 10, 11, 11, 10, 10, 10, 10, 10, 9, 9, 8,
  7, 7, 8, 7, 7, 6, 6, 5, 5, 0, 0, 0, 0, 0, 0, 0,
];
const OFFSETS = [
  0, 0, 0, 0, 0, 4096, 9216, 21504, 35840, 44032, 53248, 63488, 74752, 87040,
  93696, 100864, 104704, 106752, 108928, 113536, 115968, 118528, 119872,
  121280, 122016, 122784, 122784, 122784, 122784, 122784, 122784, 122784,
];
/* Cross-check the two tables against dictionary.c so a version bump is caught. */
{
  const src = read("common/dictionary.c");
  const grab = (label) => {
    const at = src.indexOf(`/* ${label} */`);
    if (at < 0) throw new Error(`dictionary.c: no ${label}`);
    const open = src.indexOf("{", at);
    const close = src.indexOf("}", open);
    return src.slice(open + 1, close).split(",").map((s) => s.trim()).filter(Boolean).map(Number);
  };
  const eq = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
  if (!eq(grab("size_bits_by_length"), SIZE_BITS)) throw new Error("size_bits_by_length drifted");
  if (!eq(grab("offsets_by_length"), OFFSETS)) throw new Error("offsets_by_length drifted");
  let total = 0;
  for (let len = 4; len <= 24; len++) total += (1 << SIZE_BITS[len]) * len;
  if (total !== dictBytes.length) throw new Error("word counts do not tile the dictionary");
}

/* ---- 2. transforms --------------------------------------------------- */
const tsrc = read("common/transform.c");

/* Decode the concatenated C string literal that holds every prefix/suffix. */
function cStringLiteral(src, varName) {
  const at = src.indexOf(varName);
  const eq = src.indexOf("=", at);
  const semi = src.indexOf(";", eq);
  const region = src.slice(eq + 1, semi);
  const out = [];
  let i = 0;
  let inStr = false;
  while (i < region.length) {
    const ch = region[i];
    if (!inStr) {
      if (ch === '"') inStr = true;
      else if (region.startsWith("/*", i)) i = region.indexOf("*/", i) + 1;
      i++;
      continue;
    }
    if (ch === '"') { inStr = false; i++; continue; }
    if (ch !== "\\") { out.push(ch.charCodeAt(0)); i++; continue; }
    const esc = region[i + 1];
    if (esc === "x") {
      const hex = /^[0-9a-fA-F]{1,2}/.exec(region.slice(i + 2))[0];
      out.push(parseInt(hex, 16));
      i += 2 + hex.length;
    } else if (esc >= "0" && esc <= "7") {
      const oct = /^[0-7]{1,3}/.exec(region.slice(i + 1))[0];
      out.push(parseInt(oct, 8));
      i += 1 + oct.length;
    } else {
      const simple = { n: 10, t: 9, r: 13, "\\": 92, '"': 34, "'": 39, "0": 0 };
      out.push(simple[esc] ?? esc.charCodeAt(0));
      i += 2;
    }
  }
  return Uint8Array.from(out);
}

const literal = cStringLiteral(tsrc, "kPrefixSuffix[217]");
if (literal.length !== 216) {
  throw new Error(`kPrefixSuffix decoded to ${literal.length} bytes, want 216 + implicit NUL`);
}
/* C gives the array its implicit trailing zero; entry 49 is that empty string. */
const prefixSuffix = Uint8Array.from([...literal, 0]);

const mapRegion = tsrc.slice(tsrc.indexOf("kPrefixSuffixMap[50]"));
const prefixSuffixMap = mapRegion
  .slice(mapRegion.indexOf("{") + 1, mapRegion.indexOf("}"))
  .split(",").map((s) => s.trim()).filter(Boolean).map((s) => parseInt(s, 16));
if (prefixSuffixMap.length !== 50) throw new Error("kPrefixSuffixMap is not 50 entries");

/* Each map entry points at a length byte followed by exactly that many bytes,
   and the entries tile the blob. That is a complete check on the parse. */
const pieces = prefixSuffixMap.map((off, i) => {
  const len = prefixSuffix[off];
  const end = off + 1 + len;
  const next = i + 1 < prefixSuffixMap.length ? prefixSuffixMap[i + 1] : 217;
  if (end !== next) throw new Error(`prefix/suffix ${i} at ${off}: length ${len} does not reach ${next}`);
  return Array.from(prefixSuffix.slice(off + 1, end));
});

const TYPE_NAMES = [
  "IDENTITY", "OMIT_LAST_1", "OMIT_LAST_2", "OMIT_LAST_3", "OMIT_LAST_4",
  "OMIT_LAST_5", "OMIT_LAST_6", "OMIT_LAST_7", "OMIT_LAST_8", "OMIT_LAST_9",
  "UPPERCASE_FIRST", "UPPERCASE_ALL", "OMIT_FIRST_1", "OMIT_FIRST_2",
  "OMIT_FIRST_3", "OMIT_FIRST_4", "OMIT_FIRST_5", "OMIT_FIRST_6",
  "OMIT_FIRST_7", "OMIT_FIRST_8", "OMIT_FIRST_9", "SHIFT_FIRST", "SHIFT_ALL",
];
const dataRegion = tsrc.slice(tsrc.indexOf("kTransformsData[]"));
const transformTriples = dataRegion
  .slice(dataRegion.indexOf("{") + 1, dataRegion.indexOf("};"))
  .split(",").map((s) => s.trim()).filter(Boolean)
  .map((s) => (/^\d+$/.test(s) ? Number(s) : TYPE_NAMES.indexOf(s.replace("BROTLI_TRANSFORM_", ""))));
if (transformTriples.some((v) => v < 0)) throw new Error("unknown transform type name");
if (transformTriples.length % 3) throw new Error("kTransformsData is not triples");
const transforms = [];
for (let i = 0; i < transformTriples.length; i += 3) {
  transforms.push([transformTriples[i], transformTriples[i + 1], transformTriples[i + 2]]);
}
if (transforms.length !== 121) throw new Error(`${transforms.length} transforms, want 121`);

/* ---- 3. context lookup table ---------------------------------------- */
const ctxSrc = read("common/context.c");
const ctxRegion = ctxSrc.slice(ctxSrc.indexOf("_kBrotliContextLookupTable[2048]"));
const contextLut = ctxRegion
  .slice(ctxRegion.indexOf("{") + 1, ctxRegion.indexOf("};"))
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .split(",").map((s) => s.trim()).filter(Boolean).map(Number);
if (contextLut.length !== 2048 || contextLut.some((v) => !Number.isInteger(v) || v < 0 || v > 255)) {
  throw new Error(`context lut parsed as ${contextLut.length} entries`);
}

/* ---- 4. command lookup table (for verifying our derivation) ---------- */
const cmdSrc = read("dec/prefix.h");
const cmdRegion = cmdSrc.slice(cmdSrc.indexOf("kCmdLut[BROTLI_NUM_COMMAND_SYMBOLS]"));
const cmdRows = [...cmdRegion.matchAll(/\{\s*(0x[0-9a-f]+),\s*(0x[0-9a-f]+),\s*(-?\d+),\s*(0x[0-9a-f]+),\s*(0x[0-9a-f]+),\s*(0x[0-9a-f]+)\s*\}/gi)]
  .map((m) => ({
    insertExtra: parseInt(m[1], 16),
    copyExtra: parseInt(m[2], 16),
    distanceCode: Number(m[3]),
    context: parseInt(m[4], 16),
    insertOffset: parseInt(m[5], 16),
    copyOffset: parseInt(m[6], 16),
  }));
if (cmdRows.length !== 704) throw new Error(`kCmdLut parsed as ${cmdRows.length} rows`);

/* ---- 5. emit --------------------------------------------------------- */
const b64 = (buf) => Buffer.from(buf).toString("base64");
const versionSrc = read("common/version.h");
const part = (name) => versionSrc.match(new RegExp(`BROTLI_VERSION_${name} (\\d+)`))?.[1] ?? "?";
const versionText = `${part("MAJOR")}.${part("MINOR")}.${part("PATCH")}`;

const out = `/* GENERATED by gen-data.mjs from Brotli C ${versionText} — do not edit.
   Static RFC 7932 data: the 122,784-byte dictionary, the 121 transforms and
   the 2,048-byte context lookup table. */
(function (BM) {
  BM.data = {
    brotliVersion: ${JSON.stringify(versionText)},
    dictionarySha256: ${JSON.stringify(dictSha)},
    sizeBitsByLength: ${JSON.stringify(SIZE_BITS)},
    offsetsByLength: ${JSON.stringify(OFFSETS)},
    dictionaryBase64: ${JSON.stringify(b64(dictBytes))},
    /* brotli's own compressed copy of the dictionary: a 51,687-byte stream the
       page decodes with its own decoder as a self-test. */
    dictionaryBrBase64: ${JSON.stringify(b64(dictBr))},
    transformTypeNames: ${JSON.stringify(TYPE_NAMES)},
    prefixSuffix: ${JSON.stringify(pieces.map((p) => Buffer.from(p).toString("latin1")))},
    transforms: ${JSON.stringify(transforms)},
    contextLutBase64: ${JSON.stringify(b64(Uint8Array.from(contextLut)))},
    /* kCmdLut from dec/prefix.h, kept only so the build can prove that the
       page derives the same 704 rows from the RFC's two 24-entry tables. */
    cmdLutCheck: ${JSON.stringify(cmdRows.map((r) => [r.insertExtra, r.copyExtra, r.distanceCode, r.context, r.insertOffset, r.copyOffset]))},
  };
})(globalThis.BM || (globalThis.BM = {}));
`;

writeFileSync(join(here, "data/tables.js"), out);
console.log(`wrote data/tables.js (${out.length} bytes) from brotli ${versionText}`);
console.log(`  dictionary sha256 ${dictSha}`);
console.log(`  ${transforms.length} transforms, ${pieces.length} prefix/suffix strings, ${cmdRows.length} command rows`);
