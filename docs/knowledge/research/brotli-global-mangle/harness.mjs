#!/usr/bin/env node
import { brotliCompressSync, gzipSync, constants as Z } from "node:zlib";
import { createRequire } from "node:module";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(
  "/Users/yeargun/lilscript/benchmarks/popular/package.json",
);
const acorn = require("acorn");
const here = fileURLToPath(new URL(".", import.meta.url));

const CORPORA = {
  "jquery-min":
    "/Users/yeargun/lilscript/benchmarks/popular/upstream/jquery/dist/jquery.min.js",
  "jquery-src":
    "/Users/yeargun/lilscript/benchmarks/popular/upstream/jquery/dist/jquery.js",
  "jquery-lil-raw":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-lilscript.raw.js",
  "jquery-lil-measured":
    "/Users/yeargun/lilscript/benchmarks/popular/build/jquery-measured.js",
  "jquery-lil-min":
    "/Users/yeargun/lilscript/benchmarks/popular/build/jquery-lilscript.min.js",
  "glmatrix-js-vite":
    "/Users/yeargun/lilscript/benchmarks/popular/build/gl-matrix-vite-run.mjs",
  "glmatrix-lil-vite":
    "/Users/yeargun/lilscript/benchmarks/popular/build/gl-matrix-lilscript-vite-run.mjs",
  "glmatrix-js-raw":
    "/Users/yeargun/lilscript/benchmarks/popular/build/gl-matrix-raw.mjs",
  "glmatrix-lil-raw":
    "/Users/yeargun/lilscript/benchmarks/popular/build/gl-matrix-lilscript-raw.mjs",
};

const AUDIT = {
  "audit-lean":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-lean.raw.js",
  "audit-balanced":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-balanced.raw.js",
  "audit-no-string-pool":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-no-string-pool.raw.js",
  "audit-no-reserve":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-no-reserve.raw.js",
  "audit-no-inlining":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-no-inlining.raw.js",
  "audit-positional":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-positional-aggregates.raw.js",
  "audit-function-spelling":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-function-spelling.raw.js",
  "audit-no-number-pool":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-no-number-pool.raw.js",
  "audit-readable":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-readable.raw.js",
  "audit-mangled-exports":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-audit-mangled-exports.raw.js",
};

function score(text, { q11 = true, q5 = true } = {}) {
  const bytes = Buffer.from(text, "utf8");
  const row = { raw: bytes.length, gzip9: gzipSync(bytes, { level: 9 }).length };
  if (q5) {
    row.br5 = brotliCompressSync(bytes, {
      params: {
        [Z.BROTLI_PARAM_QUALITY]: 5,
        [Z.BROTLI_PARAM_MODE]: Z.BROTLI_MODE_GENERIC,
        [Z.BROTLI_PARAM_LGWIN]: 22,
      },
    }).length;
  }
  if (q11) {
    row.br11 = brotliCompressSync(bytes, {
      params: {
        [Z.BROTLI_PARAM_QUALITY]: 11,
        [Z.BROTLI_PARAM_MODE]: Z.BROTLI_MODE_GENERIC,
        [Z.BROTLI_PARAM_LGWIN]: 22,
      },
    }).length;
  }
  return row;
}

function tokenize(code) {
  const tokens = [];
  for (const token of acorn.tokenizer(code, {
    ecmaVersion: 2022,
    allowHashBang: true,
    allowReturnOutsideFunction: true,
  })) {
    tokens.push(token);
  }
  return tokens;
}

function rewrite(code, decide) {
  const tokens = tokenize(code);
  let out = "";
  let cursor = 0;
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    const prev = tokens[i - 1];
    const next = tokens[i + 1];
    const replacement = decide(token, prev, next, i, tokens);
    if (replacement != null) {
      out += code.slice(cursor, token.start) + replacement;
      cursor = token.end;
    }
  }
  return out + code.slice(cursor);
}

function isName(token) {
  return token.type.label === "name";
}

function isKeyword(token, word) {
  return token.type.keyword === word || (isName(token) && token.value === word);
}

function afterDot(prev) {
  return prev && prev.type.label === ".";
}

function beforeColon(next) {
  return next && next.type.label === ":";
}

function localName(token, prev, next) {
  return isName(token) && !afterDot(prev) && !beforeColon(next);
}

function identFreq(code, pred = localName) {
  const freq = new Map();
  const tokens = tokenize(code);
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (!pred(token, tokens[i - 1], tokens[i + 1])) continue;
    const name = token.value;
    freq.set(name, (freq.get(name) || 0) + 1);
  }
  return [...freq.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

const SHORT = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
const FROM_FUNCTION = "eniotarslcufp";
const RARE = "qwxyzjkQWXZJK";

function remapLocals(code, mapFn) {
  return rewrite(code, (token, prev, next) => {
    if (!localName(token, prev, next)) return null;
    const nextName = mapFn(token.value);
    return nextName === token.value ? null : nextName;
  });
}

function rotateShort(code, shift) {
  return remapLocals(code, (name) => {
    if (name.length !== 1) return name;
    const i = SHORT.indexOf(name);
    if (i < 0) return name;
    return SHORT[(i + shift) % SHORT.length];
  });
}

function forceAlphabet(code, alphabet) {
  const ranked = identFreq(code).filter(([name]) => name.length <= 2);
  const map = new Map();
  ranked.forEach(([name], i) => {
    if (i < alphabet.length) map.set(name, alphabet[i]);
  });
  return remapLocals(code, (name) => map.get(name) || name);
}

function dictLocals(code, words) {
  const ranked = identFreq(code).filter(([name]) => name.length <= 2);
  const map = new Map();
  ranked.slice(0, words.length).forEach(([name], i) => map.set(name, words[i]));
  return remapLocals(code, (name) => map.get(name) || name);
}

function uniquifyShort(code) {
  const seen = new Map();
  return remapLocals(code, (name) => {
    if (name.length > 2) return name;
    const n = (seen.get(name) || 0) + 1;
    seen.set(name, n);
    return n === 1 ? name : `${name}${n}`;
  });
}

function collapseToOneLetter(code, letter = "e") {
  return remapLocals(code, (name) => (name.length === 1 ? letter : name));
}

function flipQuotes(code) {
  return rewrite(code, (token) => {
    if (token.type.label !== "string") return null;
    const raw = JSON.stringify(token.value);
    return `'${token.value.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
  });
}

function minifyBooleans(code) {
  return rewrite(code, (token) => {
    if (token.type.keyword === "true") return "!0";
    if (token.type.keyword === "false") return "!1";
    if (token.type.keyword === "undefined" || token.value === "undefined") {
      return "void 0";
    }
    return null;
  });
}

function expandBooleans(code) {
  return code
    .replaceAll("!0", "true")
    .replaceAll("!1", "false")
    .replaceAll("void 0", "undefined");
}

function swapDecl(code, from, to) {
  return rewrite(code, (token) => {
    if (token.type.keyword === from) return to;
    return null;
  });
}

function poolStrings(code, minCount = 4, minLen = 6) {
  const counts = new Map();
  for (const token of tokenize(code)) {
    if (token.type.label !== "string") continue;
    if (token.value.length < minLen) continue;
    counts.set(token.value, (counts.get(token.value) || 0) + 1);
  }
  const pooled = [...counts.entries()]
    .filter(([, n]) => n >= minCount)
    .sort((a, b) => b[1] * b[0].length - a[1] * a[0].length)
    .slice(0, 40)
    .map(([s]) => s);
  if (!pooled.length) return code;
  const index = new Map(pooled.map((s, i) => [s, i]));
  const rewritten = rewrite(code, (token) => {
    if (token.type.label !== "string") return null;
    if (!index.has(token.value)) return null;
    return `P[${index.get(token.value)}]`;
  });
  return `var P=${JSON.stringify(pooled)};${rewritten}`;
}

function prependBait(code, bait) {
  return `${bait}\n${code}`;
}

function functionBlocks(code) {
  const re = /(?:^|[;}])((?:async\s+)?function\*?)/g;
  const cuts = [0];
  let match;
  while ((match = re.exec(code))) {
    const at = match.index + match[0].length - match[1].length;
    if (at > 0) cuts.push(at);
  }
  cuts.push(code.length);
  const unique = [...new Set(cuts)].sort((a, b) => a - b);
  const parts = [];
  for (let i = 0; i < unique.length - 1; i++) {
    parts.push(code.slice(unique[i], unique[i + 1]));
  }
  return parts;
}

function reorderFunctions(code, mode) {
  const parts = functionBlocks(code);
  if (parts.length < 4) return code;
  const head = parts[0];
  const rest = parts.slice(1);
  if (mode === "reverse") return head + rest.reverse().join("");
  if (mode === "by-length") {
    return head + [...rest].sort((a, b) => a.length - b.length).join("");
  }
  if (mode === "by-prefix") {
    return head + [...rest].sort((a, b) => a.slice(0, 24).localeCompare(b.slice(0, 24))).join("");
  }
  return code;
}

function chunkIndependent(code, bytes) {
  let total = 0;
  for (let i = 0; i < code.length; i += bytes) {
    total += score(code.slice(i, i + bytes)).br11;
  }
  return { raw: code.length, gzip9: null, br5: null, br11: total, note: `independent ${bytes}-byte chunks` };
}

function dotToBracket(code, props) {
  const set = new Set(props);
  return rewrite(code, (token, prev) => {
    if (!isName(token) || !afterDot(prev) || !set.has(token.value)) return null;
    return `["${token.value}"]`;
  });
}

function mutationsFor(name, code) {
  const out = [];
  const add = (id, text, note) => out.push({ id, text, note });
  add("baseline", code, "unmodified artifact");
  add("quotes-single", flipQuotes(code), "every string literal forced to single quotes");
  add("bool-minify", minifyBooleans(code), "true/false/undefined -> !0/!1/void 0");
  add("bool-expand", expandBooleans(code), "reverse of bool-minify; may over-replace");
  add("var-to-let", swapDecl(code, "var", "let"), "declaration keyword swap");
  add("let-to-var", swapDecl(code, "let", "var"), "declaration keyword swap");
  add("const-to-var", swapDecl(code, "const", "var"), "declaration keyword swap");
  add("rotate-short-1", rotateShort(code, 1), "permute 1-char locals along a-zA-Z_$");
  add("rotate-short-13", rotateShort(code, 13), "far permutation of 1-char locals");
  add("alphabet-function-letters", forceAlphabet(code, FROM_FUNCTION), "assign hottest short locals to letters from function/return");
  add("alphabet-rare", forceAlphabet(code, RARE), "assign hottest short locals to rare letters");
  add("locals-as-length-index-value", dictLocals(code, ["length", "index", "value", "name", "type", "data"]), "hottest short locals become ROM words");
  add("locals-as-function-return", dictLocals(code, ["function", "return", "undefined", "prototype", "document", "window"]), "hottest short locals become long ROM words");
  add("uniquify-short", uniquifyShort(code), "break cross-scope reuse of 1-2 char names");
  add("collapse-to-e", collapseToOneLetter(code, "e"), "illegal: every 1-char local becomes e");
  add("pool-strings-4x6", poolStrings(code, 4, 6), "array-pool repeated strings");
  add("pool-strings-8x8", poolStrings(code, 8, 8), "stricter pool");
  add("bait-function-return", prependBait(code, "function(){return;}"), "ROM-phrase preamble");
  add("bait-javascript-type", prependBait(code, 'var __="type=\\"text/javascript\\""'), "exact 22-byte ROM phrase as dummy");
  add("bait-unique", prependBait(code, "function(){return;}".replaceAll("function", "qwxkzzzz")), "same shape, unique word");
  add("fn-reverse", reorderFunctions(code, "reverse"), "reverse function declaration order");
  add("fn-by-length", reorderFunctions(code, "by-length"), "shortest functions first");
  add("fn-by-prefix", reorderFunctions(code, "by-prefix"), "cluster similar function prefixes");
  add("dot-length-bracket", dotToBracket(code, ["length", "prototype", "name", "type"]), ".length -> [\"length\"]");
  return out;
}

function main() {
  const started = Date.now();
  const report = {
    generatedAt: new Date().toISOString(),
    note: "Diagnostic Node zlib brotli 1.1.0 generic lgwin 22. Not lilscript-codec. Mutations are compression-only; several are semantically illegal.",
    corpora: {},
    audits: {},
    monaco: {},
  };

  for (const [name, path] of Object.entries(CORPORA)) {
    const code = readFileSync(path, "utf8");
    console.error("corpus", name, code.length);
    const variants = mutationsFor(name, code);
    const rows = [];
    for (const variant of variants) {
      const sizes = score(variant.text);
      rows.push({
        id: variant.id,
        note: variant.note,
        ...sizes,
        dBr11: null,
      });
      console.error(" ", variant.id, sizes);
    }
    const base = rows.find((r) => r.id === "baseline");
    for (const row of rows) row.dBr11 = row.br11 - base.br11;
    report.corpora[name] = {
      path,
      file: basename(path),
      baseline: base,
      rows,
    };
  }

  for (const [name, path] of Object.entries(AUDIT)) {
    const code = readFileSync(path, "utf8");
    console.error("audit", name, code.length);
    report.audits[name] = { path, file: basename(path), ...score(code) };
  }

  const monaco = readFileSync(
    "/Users/yeargun/lilscript/benchmarks/popular/apps/monaco/lil/ide.js",
    "utf8",
  );
  const cut = monaco.lastIndexOf(";\n", 400_000);
  const slice = monaco.slice(0, cut > 1000 ? cut + 1 : 350_000);
  console.error("monaco-lil-ide", monaco.length, "slice", slice.length);
  report.monaco.fullBaseline = { raw: monaco.length, ...score(monaco, { q5: false }) };
  const monacoVariants = [
    ["baseline", slice],
    ["quotes-single", flipQuotes(slice)],
    ["bool-minify", minifyBooleans(slice)],
    ["rotate-short-1", rotateShort(slice, 1)],
    ["locals-as-length-index-value", dictLocals(slice, ["length", "index", "value", "name"])],
    ["pool-strings-4x6", poolStrings(slice, 4, 6)],
    ["fn-by-prefix", reorderFunctions(slice, "by-prefix")],
    ["bait-javascript-type", prependBait(slice, 'var __="type=\\"text/javascript\\""')],
  ];
  report.monaco.slice400k = [];
  for (const [id, text] of monacoVariants) {
    const sizes = score(text);
    report.monaco.slice400k.push({ id, ...sizes });
    console.error(" monaco slice", id, sizes);
  }
  const sliceBase = report.monaco.slice400k[0];
  for (const row of report.monaco.slice400k) row.dBr11 = row.br11 - sliceBase.br11;

  report.chunks = {
    "jquery-lil-raw-32k": chunkIndependent(readFileSync(CORPORA["jquery-lil-raw"], "utf8"), 32768),
    "jquery-lil-raw-64k": chunkIndependent(readFileSync(CORPORA["jquery-lil-raw"], "utf8"), 65536),
    "jquery-lil-raw-whole": score(readFileSync(CORPORA["jquery-lil-raw"], "utf8")),
  };

  report.elapsedMs = Date.now() - started;
  mkdirSync(here, { recursive: true });
  writeFileSync(join(here, "results.json"), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ elapsedMs: report.elapsedMs, corpora: Object.keys(report.corpora) }, null, 2));
}

main();
