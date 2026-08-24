#!/usr/bin/env node
/* Self-tests for the brotli-machine engine: the decoder is checked against
   streams produced by the real Brotli library, and the encoder's output is
   checked by handing it back to the real library. */
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";
import { loadEngine } from "./engine.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const BM = loadEngine();
const enc = new TextEncoder();
const dec = new TextDecoder();

let pass = 0, fail = 0;
let category = "other";
const byCategory = {};
function group(name) { category = name; console.log(name); }
function check(name, ok, note = "") {
  byCategory[category] = (byCategory[category] || 0) + 1;
  if (ok) { pass++; if (process.env.VERBOSE) console.log(`  ok   ${name} ${note}`); }
  else { fail++; console.log(`  FAIL ${name} ${note}`); }
}

const samples = {
  empty: "",
  one: "a",
  hello: "Hello, hello, hello world!",
  repeat: "abcabcabcabcabcabcabcabcabcabc",
  html: `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>The quick brown fox</title></head><body><div class="container"><p>Hello world</p></div></body></html>`,
  js: readFileSync(join(here, "src/30-huffman.js"), "utf8"),
  bigjs: readFileSync(join(here, "src/50-decoder.js"), "utf8"),
  binary: String.fromCharCode(...Array.from({ length: 4096 }, (_, i) => (i * 37) & 0xff)),
  utf8: "üñïçödé — ünïcödé tèxt, répèated: üñïçödé — ünïcödé tèxt".repeat(20),
  json: JSON.stringify(Object.fromEntries(Array.from({ length: 200 }, (_, i) => [`key_${i}`, { id: i, name: `item ${i}`, tags: ["alpha", "beta"] }]))),
  lorem: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(40),
};

group("decoder vs. real brotli streams");
for (const [name, text] of Object.entries(samples)) {
  const input = name === "binary" ? Uint8Array.from(text, (c) => c.charCodeAt(0)) : enc.encode(text);
  for (const quality of [0, 1, 5, 9, 11]) {
    for (const lgwin of [10, 16, 22, 24]) {
      const compressed = zlib.brotliCompressSync(Buffer.from(input), {
        params: {
          [zlib.constants.BROTLI_PARAM_QUALITY]: quality,
          [zlib.constants.BROTLI_PARAM_LGWIN]: lgwin,
          [zlib.constants.BROTLI_PARAM_SIZE_HINT]: input.length,
        },
      });
      let got;
      try {
        got = BM.decode(new Uint8Array(compressed), { trace: false });
      } catch (e) {
        check(`${name} q${quality} w${lgwin}`, false, `threw ${e.message}`);
        continue;
      }
      const same = got.output.length === input.length && got.output.every((b, i) => b === input[i]);
      check(`${name} q${quality} w${lgwin}`, same,
        same ? "" : `${got.output.length} bytes out, want ${input.length}`);
    }
  }
}

/* Text modes that push the format into its corners. */
group("decoder corner cases");
{
  const cases = [
    ["uncompressed metablock", zlib.brotliCompressSync(Buffer.from("x".repeat(3) + String.fromCharCode(...Array.from({length: 2000}, () => Math.floor(Math.random() * 256)))), { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 0 } })],
    ["large text", zlib.brotliCompressSync(readFileSync(join(here, "gen-data.mjs")), { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11 } })],
  ];
  for (const [name, buf] of cases) {
    const expected = zlib.brotliDecompressSync(buf);
    const got = BM.decode(new Uint8Array(buf), { trace: false });
    check(name, Buffer.compare(Buffer.from(got.output), expected) === 0);
  }
}

/* Brotli's own compressed dictionary: 51,687 bytes in, 122,784 out. */
group("decoder on brotli's compressed dictionary");
{
  const br = BM.base64ToBytes(BM.data.dictionaryBrBase64);
  const got = BM.decode(br, { trace: false });
  const want = BM.base64ToBytes(BM.data.dictionaryBase64);
  check("dictionary.bin.br", got.output.length === want.length && got.output.every((b, i) => b === want[i]),
    `${got.output.length} bytes`);
}

if (BM.encode) {
  group("encoder round trip through the real library");
  for (const [name, text] of Object.entries(samples)) {
    const input = name === "binary" ? Uint8Array.from(text, (c) => c.charCodeAt(0)) : enc.encode(text);
    let result;
    try {
      result = BM.encode(input, { trace: false });
    } catch (e) {
      check(`encode ${name}`, false, `threw ${e.message}\n${e.stack}`);
      continue;
    }
    let round;
    try {
      round = zlib.brotliDecompressSync(Buffer.from(result.bytes));
    } catch (e) {
      check(`encode ${name}`, false, `real brotli rejected the stream: ${e.message}`);
      continue;
    }
    const ok = Buffer.compare(round, Buffer.from(input)) === 0;
    const ours = BM.decode(result.bytes, { trace: false });
    const okSelf = ours.output.length === input.length && ours.output.every((b, i) => b === input[i]);
    const ref = zlib.brotliCompressSync(Buffer.from(input), { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11 } });
    check(`encode ${name}`, ok && okSelf,
      `${input.length} -> ${result.bytes.length} bytes (brotli q11: ${ref.length})`);
    if (process.env.SIZES) {
      console.log(`       ${name}: ${input.length} raw, ${result.bytes.length} ours, ${ref.length} brotli q11, ${zlib.gzipSync(Buffer.from(input), {level:9}).length} gzip-9`);
    }
  }
}

/* Parameter combinations, and then fuzz: the encoder has to produce a legal
   stream whatever the knobs say and whatever the bytes are. */
if (BM.encode) {
  group("encoder across parameter combinations");
  const subject = enc.encode(samples.js.slice(0, 3000));
  for (const literalTrees of [1, 2, 4, 8, 16]) {
    for (const contextMode of [0, 1, 2, 3]) {
      for (const [useDictionary, lazy] of [[true, true], [false, true], [true, false]]) {
        const plugins = {
          chooseParams: (c) => Object.assign(BM.defaultPlugins.chooseParams(c),
            { literalTrees, contextMode, useDictionary, lazy }),
        };
        let ok = false, note = "";
        try {
          const r = BM.encode(subject, { trace: false, plugins });
          ok = Buffer.compare(zlib.brotliDecompressSync(Buffer.from(r.bytes)), Buffer.from(subject)) === 0;
          note = `${r.bytes.length} bytes`;
        } catch (e) { note = e.message; }
        check(`trees=${literalTrees} mode=${contextMode} dict=${useDictionary} lazy=${lazy}`, ok, note);
      }
    }
  }

  group("encoder fuzz");
  let seed = 20260823;
  const rand = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
  const alphabets = [
    "ab", "abcdefghijklmnopqrstuvwxyz", "0123456789", " \n\t{}();=<>/\"'",
    "<div class=\"container\"> the of and function return ",
    String.fromCharCode(...Array.from({ length: 256 }, (_, i) => i)),
  ];
  for (let trial = 0; trial < 60; trial++) {
    const alphabet = alphabets[Math.floor(rand() * alphabets.length)];
    const length = Math.floor(rand() * rand() * 4000) + (trial % 7);
    let text = "";
    while (text.length < length) {
      if (text.length > 8 && rand() < 0.3) {
        const start = Math.floor(rand() * text.length);
        text += text.slice(start, start + 1 + Math.floor(rand() * 60));
      } else {
        text += alphabet[Math.floor(rand() * alphabet.length)];
      }
    }
    const input = Uint8Array.from(text.slice(0, length), (c) => c.charCodeAt(0) & 0xff);
    let ok = false, note = "";
    try {
      const r = BM.encode(input, { trace: false });
      const round = zlib.brotliDecompressSync(Buffer.from(r.bytes));
      const mine = BM.decode(r.bytes, { trace: false });
      ok = Buffer.compare(round, Buffer.from(input)) === 0 &&
           Buffer.compare(Buffer.from(mine.output), Buffer.from(input)) === 0;
      note = `${input.length} -> ${r.bytes.length}`;
    } catch (e) { note = `${input.length} bytes: ${e.message}`; }
    check(`fuzz ${trial}`, ok, note);
  }
}

console.log(`\n${pass} passed, ${fail} failed`);
console.log("SUMMARY " + JSON.stringify({ pass, fail, byCategory }));
process.exit(fail ? 1 : 0);
