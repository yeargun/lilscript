#!/usr/bin/env node
import { brotliCompressSync, gzipSync, constants as Z } from "node:zlib";
import { createRequire } from "node:module";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
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
  "jquery-lil-raw":
    "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery/jquery-lilscript.raw.js",
  "jquery-lil-min":
    "/Users/yeargun/lilscript/benchmarks/popular/build/jquery-lilscript.min.js",
  "glmatrix-lil-vite":
    "/Users/yeargun/lilscript/benchmarks/popular/build/gl-matrix-lilscript-vite-run.mjs",
  "glmatrix-js-vite":
    "/Users/yeargun/lilscript/benchmarks/popular/build/gl-matrix-vite-run.mjs",
};

function score(text) {
  const bytes = Buffer.from(text, "utf8");
  return {
    raw: bytes.length,
    gzip9: gzipSync(bytes, { level: 9 }).length,
    br5: brotliCompressSync(bytes, {
      params: {
        [Z.BROTLI_PARAM_QUALITY]: 5,
        [Z.BROTLI_PARAM_MODE]: Z.BROTLI_MODE_GENERIC,
        [Z.BROTLI_PARAM_LGWIN]: 22,
      },
    }).length,
    br11: brotliCompressSync(bytes, {
      params: {
        [Z.BROTLI_PARAM_QUALITY]: 11,
        [Z.BROTLI_PARAM_MODE]: Z.BROTLI_MODE_GENERIC,
        [Z.BROTLI_PARAM_LGWIN]: 22,
      },
    }).length,
  };
}

function tokenize(code) {
  return [...acorn.tokenizer(code, { ecmaVersion: 2022, allowHashBang: true, allowReturnOutsideFunction: true })];
}

function rewrite(code, decide) {
  const tokens = tokenize(code);
  let out = "";
  let cursor = 0;
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    const replacement = decide(token, tokens[i - 1], tokens[i + 1]);
    if (replacement != null) {
      out += code.slice(cursor, token.start) + replacement;
      cursor = token.end;
    }
  }
  return out + code.slice(cursor);
}

function localName(token, prev, next) {
  if (token.type.label !== "name") return false;
  if (prev && prev.type.label === ".") return false;
  if (next && next.type.label === ":") return false;
  return true;
}

function identFreq(code) {
  const freq = new Map();
  const tokens = tokenize(code);
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (!localName(token, tokens[i - 1], tokens[i + 1])) continue;
    freq.set(token.value, (freq.get(token.value) || 0) + 1);
  }
  return [...freq.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

function remapLocals(code, map) {
  return rewrite(code, (token, prev, next) => {
    if (!localName(token, prev, next)) return null;
    return map.get(token.value) ?? null;
  });
}

function letterHist(text) {
  const hist = Object.create(null);
  for (const ch of text) {
    if (/[A-Za-z_$]/.test(ch)) hist[ch] = (hist[ch] || 0) + 1;
  }
  return Object.entries(hist).sort((a, b) => b[1] - a[1]);
}

function swapDecl(code, from, to) {
  return rewrite(code, (token) => (token.type.keyword === from ? to : null));
}

function forceAlphabet(code, alphabet) {
  const ranked = identFreq(code).filter(([name]) => name.length <= 2);
  const map = new Map();
  ranked.forEach(([name], i) => {
    if (i < alphabet.length) map.set(name, alphabet[i]);
  });
  return remapLocals(code, map);
}

function minifyBooleans(code) {
  return rewrite(code, (token) => {
    if (token.type.keyword === "true") return "!0";
    if (token.type.keyword === "false") return "!1";
    if (token.type.keyword === "undefined" || token.value === "undefined") return "void 0";
    return null;
  });
}

function statsFor(name, code) {
  const freq = identFreq(code);
  const short1 = freq.filter(([n]) => n.length === 1);
  const short2 = freq.filter(([n]) => n.length === 2);
  const uses1 = short1.reduce((s, [, n]) => s + n, 0);
  const letters = letterHist(code).slice(0, 16);
  return {
    name,
    raw: code.length,
    uniqueLocals: freq.length,
    unique1: short1.length,
    unique2: short2.length,
    uses1,
    reuse1: short1.length ? +(uses1 / short1.length).toFixed(1) : 0,
    topLocals: freq.slice(0, 12).map(([n, c]) => ({ n, c })),
    topLetters: letters.map(([ch, c]) => ({ ch, c })),
  };
}

function inversions() {
  const report = JSON.parse(readFileSync(join(here, "results.json"), "utf8"));
  const rows = [];
  for (const [corpus, c] of Object.entries(report.corpora)) {
    const b = c.baseline;
    for (const row of c.rows) {
      if (row.id === "baseline") continue;
      const dGz = row.gzip9 - b.gzip9;
      const d5 = row.br5 - b.br5;
      const d11 = row.br11 - b.br11;
      const signs = [Math.sign(dGz), Math.sign(d5), Math.sign(d11)];
      const disagree =
        signs.some((s) => s !== 0) &&
        !(signs[0] === signs[1] && signs[1] === signs[2]);
      if (!disagree) continue;
      rows.push({ corpus, id: row.id, dGz, d5, d11 });
    }
  }
  return rows.sort((a, b) => Math.abs(b.d11) + Math.abs(b.dGz) - (Math.abs(a.d11) + Math.abs(a.dGz)));
}

function main() {
  const extra = {
    generatedAt: new Date().toISOString(),
    note: "Follow-up probes. Node zlib Brotli 1.1.0 generic q11. Not lilscript-codec.",
    stats: {},
    surgical: {},
    moreAudits: {},
    inversions: inversions(),
  };

  for (const [name, path] of Object.entries(CORPORA)) {
    const code = readFileSync(path, "utf8");
    extra.stats[name] = statsFor(name, code);
    console.error("stats", name, extra.stats[name].topLocals);

    const hottest = extra.stats[name].topLocals[0]?.n;
    const variants = [];
    const add = (id, text) => {
      const sizes = score(text);
      variants.push({ id, ...sizes });
      console.error(" ", id, sizes);
    };
    add("baseline", code);
    if (hottest && hottest.length <= 2) {
      for (const letter of ["e", "n", "t", "a", "q", "x"]) {
        if (letter === hottest) continue;
        add(`hottest-to-${letter}`, remapLocals(code, new Map([[hottest, letter]])));
      }
    }
    add("alphabet-eni", forceAlphabet(code, "eniotarslcufp"));
    add("alphabet-eni+let", swapDecl(forceAlphabet(code, "eniotarslcufp"), "var", "let"));
    add("alphabet-eni+bool", minifyBooleans(forceAlphabet(code, "eniotarslcufp")));
    add("let-only", swapDecl(code, "var", "let"));
    const base = variants[0];
    for (const row of variants) {
      row.dBr11 = row.br11 - base.br11;
      row.dGzip = row.gzip9 - base.gzip9;
    }
    extra.surgical[name] = variants;
  }

  const auditDir = "/Users/yeargun/lilscript/benchmarks/popular/ports/jquery";
  for (const file of readdirSync(auditDir).filter((f) => f.startsWith("jquery-audit-") && f.endsWith(".raw.js"))) {
    const path = join(auditDir, file);
    if (!existsSync(path)) continue;
    const code = readFileSync(path, "utf8");
    const sizes = score(code);
    extra.moreAudits[file] = { file: basename(file), ...sizes };
    console.error("audit", file, sizes);
  }

  writeFileSync(join(here, "extra.json"), JSON.stringify(extra, null, 2));
  console.log(JSON.stringify({
    stats: Object.keys(extra.stats),
    inversions: extra.inversions.length,
    audits: Object.keys(extra.moreAudits).length,
  }, null, 2));
}

main();
