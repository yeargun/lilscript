#!/usr/bin/env node
import { brotliCompressSync, gzipSync, constants as Z } from "node:zlib";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL(".", import.meta.url));
const dictBin = "/tmp/brotli-dict.bin";

function ensureDictionary() {
  if (existsSync(dictBin) && readFileSync(dictBin).length === 122784) {
    return readFileSync(dictBin);
  }
  const src = join(here, "dump-brotli-dict.c");
  const exe = "/tmp/dump-brotli-dict";
  execFileSync("clang", ["-O2", "-o", exe, src, "-L/opt/homebrew/lib", "-lbrotlicommon"]);
  const bin = execFileSync(exe);
  if (bin.length !== 122784) {
    throw new Error(`dictionary dump was ${bin.length} bytes`);
  }
  writeFileSync(dictBin, bin);
  return bin;
}

const SIZE_BITS = [
  0, 0, 0, 0, 10, 10, 11, 11, 10, 10, 10, 10, 10, 9, 9, 8, 7, 7, 8, 7, 7, 6, 6,
  5, 5,
];
const OFFSETS = [
  0, 0, 0, 0, 0, 4096, 9216, 21504, 35840, 44032, 53248, 63488, 74752, 87040,
  93696, 100864, 104704, 106752, 108928, 113536, 115968, 118528, 119872, 121280,
  122016,
];

function loadDictionaryWords(bin) {
  const words = [];
  const byExact = new Map();
  for (let length = 4; length <= 24; length++) {
    const n = 1 << SIZE_BITS[length];
    const off = OFFSETS[length];
    for (let i = 0; i < n; i++) {
      const raw = bin.subarray(off + i * length, off + (i + 1) * length);
      const text = raw.toString("latin1");
      words.push(text);
      if (!byExact.has(text)) byExact.set(text, []);
      byExact.get(text).push({ length, index: i });
    }
  }
  return { words, byExact };
}

const PREFIX_SUFFIX = Buffer.from(
  "\u0001 \u0002, \u0008 of the \u0004 of \u0002s \u0001.\u0005 and \u0004 in \u0001\"\u0004 to \u0002\">\u0001\n\u0002. \u0001]\u0005 for \u0003 a \u0006 that \u0001'\u0006 with \u0006 from \u0004 by \u0001(\u0006. The \u0004 on \u0004 as \u0004 is \u0004ing \u0002\n\t\u0001:\u0003ed \u0002=\"\u0004 at \u0003ly \u0001,\u0002='\u0005.com/\u0007. This \u0005 not \u0003er \u0003al \u0004ful \u0004ive \u0005less \u0004est \u0004ize \u0002\u00c2\u00a0\u0004ous \u0005 the \u0002e ",
  "latin1",
);

const PREFIX_SUFFIX_MAP = [
  0x00, 0x02, 0x05, 0x0e, 0x13, 0x16, 0x18, 0x1e, 0x23, 0x25, 0x2a, 0x2d, 0x2f,
  0x32, 0x34, 0x3a, 0x3e, 0x45, 0x47, 0x4e, 0x55, 0x5a, 0x5c, 0x63, 0x68, 0x6d,
  0x72, 0x77, 0x7a, 0x7c, 0x80, 0x83, 0x88, 0x8c, 0x8e, 0x91, 0x97, 0x9f, 0xa5,
  0xa9, 0xad, 0xb2, 0xb7, 0xbd, 0xc2, 0xc7, 0xca, 0xcf, 0xd5, 0xd8,
];

function affix(index) {
  const off = PREFIX_SUFFIX_MAP[index];
  const len = PREFIX_SUFFIX[off];
  return PREFIX_SUFFIX.subarray(off + 1, off + 1 + len).toString("latin1");
}

const IDENTITY = 0;
const OMIT_LAST = (n) => n;
const UPPERCASE_FIRST = 10;
const UPPERCASE_ALL = 11;
const OMIT_FIRST = (n) => 11 + n;

const TRANSFORMS = [
  [49, IDENTITY, 49],
  [49, IDENTITY, 0],
  [0, IDENTITY, 0],
  [49, OMIT_FIRST(1), 49],
  [49, UPPERCASE_FIRST, 0],
  [49, IDENTITY, 47],
  [0, IDENTITY, 49],
  [4, IDENTITY, 0],
  [49, IDENTITY, 3],
  [49, UPPERCASE_FIRST, 49],
  [49, IDENTITY, 6],
  [49, OMIT_FIRST(2), 49],
  [49, OMIT_LAST(1), 49],
  [1, IDENTITY, 0],
  [49, IDENTITY, 1],
  [0, UPPERCASE_FIRST, 0],
  [49, IDENTITY, 7],
  [49, IDENTITY, 9],
  [48, IDENTITY, 0],
  [49, IDENTITY, 8],
  [49, IDENTITY, 5],
  [49, IDENTITY, 10],
  [49, IDENTITY, 11],
  [49, OMIT_LAST(3), 49],
  [49, IDENTITY, 13],
  [49, IDENTITY, 14],
  [49, OMIT_FIRST(3), 49],
  [49, OMIT_LAST(2), 49],
  [49, IDENTITY, 15],
  [49, IDENTITY, 16],
  [0, UPPERCASE_FIRST, 49],
  [49, IDENTITY, 12],
  [5, IDENTITY, 49],
  [0, IDENTITY, 1],
  [49, OMIT_FIRST(4), 49],
  [49, IDENTITY, 18],
  [49, IDENTITY, 17],
  [49, IDENTITY, 19],
  [49, IDENTITY, 20],
  [49, OMIT_FIRST(5), 49],
  [49, OMIT_FIRST(6), 49],
  [47, IDENTITY, 49],
  [49, OMIT_LAST(4), 49],
  [49, IDENTITY, 22],
  [49, UPPERCASE_ALL, 49],
  [49, IDENTITY, 23],
  [49, IDENTITY, 24],
  [49, IDENTITY, 25],
  [49, OMIT_LAST(7), 49],
  [49, OMIT_LAST(1), 26],
  [49, IDENTITY, 27],
  [49, IDENTITY, 28],
  [0, IDENTITY, 12],
  [49, IDENTITY, 29],
  [49, OMIT_FIRST(9), 49],
  [49, OMIT_FIRST(7), 49],
  [49, OMIT_LAST(6), 49],
  [49, IDENTITY, 21],
  [49, UPPERCASE_FIRST, 1],
  [49, OMIT_LAST(8), 49],
  [49, IDENTITY, 31],
  [49, IDENTITY, 32],
  [47, IDENTITY, 3],
  [49, OMIT_LAST(5), 49],
  [49, OMIT_LAST(9), 49],
  [0, UPPERCASE_FIRST, 1],
  [49, UPPERCASE_FIRST, 8],
  [5, IDENTITY, 21],
  [49, UPPERCASE_ALL, 0],
  [49, UPPERCASE_FIRST, 10],
  [49, IDENTITY, 30],
  [0, IDENTITY, 5],
  [35, IDENTITY, 49],
  [47, IDENTITY, 2],
  [49, UPPERCASE_FIRST, 17],
  [49, IDENTITY, 36],
  [49, IDENTITY, 33],
  [5, IDENTITY, 0],
  [49, UPPERCASE_FIRST, 21],
  [49, UPPERCASE_FIRST, 5],
  [49, IDENTITY, 37],
  [0, IDENTITY, 30],
  [49, IDENTITY, 38],
  [0, UPPERCASE_ALL, 0],
  [49, IDENTITY, 39],
  [0, UPPERCASE_ALL, 49],
  [49, IDENTITY, 34],
  [49, UPPERCASE_ALL, 8],
  [49, UPPERCASE_FIRST, 12],
  [0, IDENTITY, 21],
  [49, IDENTITY, 40],
  [0, UPPERCASE_FIRST, 12],
  [49, IDENTITY, 41],
  [49, IDENTITY, 42],
  [49, UPPERCASE_ALL, 17],
  [49, IDENTITY, 43],
  [0, UPPERCASE_FIRST, 5],
  [49, UPPERCASE_ALL, 10],
  [0, IDENTITY, 34],
  [49, UPPERCASE_FIRST, 33],
  [49, IDENTITY, 44],
  [49, UPPERCASE_ALL, 5],
  [45, IDENTITY, 49],
  [0, IDENTITY, 33],
  [49, UPPERCASE_FIRST, 30],
  [49, UPPERCASE_ALL, 30],
  [49, IDENTITY, 46],
  [49, UPPERCASE_ALL, 1],
  [49, UPPERCASE_FIRST, 34],
  [0, UPPERCASE_FIRST, 33],
  [0, UPPERCASE_ALL, 30],
  [0, UPPERCASE_ALL, 1],
  [49, UPPERCASE_ALL, 33],
  [49, UPPERCASE_ALL, 21],
  [49, UPPERCASE_ALL, 12],
  [0, UPPERCASE_ALL, 5],
  [49, UPPERCASE_ALL, 34],
  [0, UPPERCASE_ALL, 12],
  [0, UPPERCASE_FIRST, 30],
  [0, UPPERCASE_ALL, 34],
  [0, UPPERCASE_FIRST, 34],
];

function toUpperFirst(word) {
  if (!word) return word;
  const c = word.charCodeAt(0);
  if (c >= 97 && c <= 122) return String.fromCharCode(c ^ 32) + word.slice(1);
  return word;
}

function toUpperAll(word) {
  let out = "";
  for (let i = 0; i < word.length; i++) {
    const c = word.charCodeAt(i);
    out += c >= 97 && c <= 122 ? String.fromCharCode(c ^ 32) : word[i];
  }
  return out;
}

function applyTransform(word, prefixIdx, type, suffixIdx) {
  let body = word;
  if (type >= 1 && type <= 9) body = word.slice(0, Math.max(0, word.length - type));
  else if (type >= 12 && type <= 20) body = word.slice(type - 11);
  else if (type === UPPERCASE_FIRST) body = toUpperFirst(word);
  else if (type === UPPERCASE_ALL) body = toUpperAll(word);
  return affix(prefixIdx) + body + affix(suffixIdx);
}

function describeTransform(prefixIdx, type, suffixIdx) {
  const names = {
    0: "identity",
    10: "uppercase-first",
    11: "uppercase-all",
  };
  let kind = names[type];
  if (!kind && type >= 1 && type <= 9) kind = `omit-last-${type}`;
  if (!kind && type >= 12 && type <= 20) kind = `omit-first-${type - 11}`;
  const pre = JSON.stringify(affix(prefixIdx));
  const suf = JSON.stringify(affix(suffixIdx));
  return `${pre} + ${kind} + ${suf}`;
}

function findDictionaryHits(target, words) {
  const hits = [];
  if (target.length < 1 || target.length > 40) return hits;
  for (const word of words) {
    for (let t = 0; t < TRANSFORMS.length; t++) {
      const [p, type, s] = TRANSFORMS[t];
      const out = applyTransform(word, p, type, s);
      if (out === target) {
        hits.push({
          word,
          transform: describeTransform(p, type, s),
          transformIndex: t,
        });
        if (hits.length >= 8) return hits;
      }
    }
  }
  return hits;
}

function brotliNode(bytes, { quality = 11, mode = Z.BROTLI_MODE_GENERIC, lgwin = 22, disableContext = false } = {}) {
  const params = {
    [Z.BROTLI_PARAM_QUALITY]: quality,
    [Z.BROTLI_PARAM_MODE]: mode,
    [Z.BROTLI_PARAM_LGWIN]: lgwin,
  };
  if (disableContext) params[Z.BROTLI_PARAM_DISABLE_LITERAL_CONTEXT_MODELING] = 1;
  return brotliCompressSync(bytes, { params }).length;
}

function gzipNode(bytes) {
  return gzipSync(bytes, { level: 9 }).length;
}

function brotliCli(bytes, quality = 11) {
  return execFileSync("brotli", ["-q", String(quality), "-c"], {
    input: bytes,
    maxBuffer: 16 * 1024 * 1024,
  }).length;
}

function gzipCli(bytes) {
  return execFileSync("gzip", ["-9", "-n", "-c"], {
    input: bytes,
    maxBuffer: 16 * 1024 * 1024,
  }).length;
}

function brotliPython(bytes, quality = 11, mode = "MODE_GENERIC") {
  const script = `import sys, brotli
data = sys.stdin.buffer.read()
sys.stdout.buffer.write(brotli.compress(data, quality=${quality}, mode=brotli.${mode}))
`;
  return execFileSync("python3", ["-c", script], {
    input: bytes,
    maxBuffer: 16 * 1024 * 1024,
  }).length;
}

function sizes(text, extra = {}) {
  const bytes = Buffer.from(text, "utf8");
  const row = {
    raw: bytes.length,
    gzip9: gzipNode(bytes),
    br11g: brotliNode(bytes, { quality: 11, mode: Z.BROTLI_MODE_GENERIC }),
    br11t: brotliNode(bytes, { quality: 11, mode: Z.BROTLI_MODE_TEXT }),
    br11f: brotliNode(bytes, { quality: 11, mode: Z.BROTLI_MODE_FONT }),
    br5g: brotliNode(bytes, { quality: 5, mode: Z.BROTLI_MODE_GENERIC }),
    br0g: brotliNode(bytes, { quality: 0, mode: Z.BROTLI_MODE_GENERIC }),
    ...extra,
  };
  return row;
}

function sizesFull(text) {
  const bytes = Buffer.from(text, "utf8");
  return {
    ...sizes(text),
    brCli11: brotliCli(bytes, 11),
    brCli5: brotliCli(bytes, 5),
    gzCli: gzipCli(bytes),
    brPy11: brotliPython(bytes, 11, "MODE_GENERIC"),
    brPyText: brotliPython(bytes, 11, "MODE_TEXT"),
    brNoCtx: brotliNode(bytes, { disableContext: true }),
    qualities: Object.fromEntries(
      [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((q) => [
        q,
        {
          generic: brotliNode(bytes, { quality: q, mode: Z.BROTLI_MODE_GENERIC }),
          text: brotliNode(bytes, { quality: q, mode: Z.BROTLI_MODE_TEXT }),
        },
      ]),
    ),
  };
}

function wrapUnique(inner) {
  const noise = Array.from({ length: 40 }, (_, i) => {
    const id = `q${i.toString(36)}x`;
    return `function ${id}(z${i}){return z${i}+${i}}`;
  }).join("");
  return `${noise}${inner}${noise}`;
}

function wrapJsLike(inner) {
  return `function render(document,window){if(typeof document==="undefined")return;var length=window.length;return document}${inner}`;
}

function repeatBlock(text, n) {
  return Array.from({ length: n }, () => text).join("\n");
}

const userConst = `{
const x = 5;
const y = "const";
let b = "let";
var a = { xd: 5, yes: "let"};
a["const"] = 5
}`;

const userLorem = `{
const x = 5;
const y = "const";
let b = "let";
var a = { xd: 5, yes: "let"};
a["lorem"] = 5
}`;

const cases = [];

function add(group, name, text, note = "") {
  cases.push({ group, name, text, note });
}

add("user-snippet", "a[\"const\"]=5  (keyword reused as property)", userConst, "Your first snippet. The last key copies the already-emitted keyword const.");
add("user-snippet", "a[\"lorem\"]=5  (unique property)", userLorem, "Same bytes except the last string is a unique word.");
add("user-snippet", "a.const=5  (dot, reserved word is a legal key)", `{
const x = 5;
const y = "const";
let b = "let";
var a = { xd: 5, yes: "let"};
a.const = 5
}`);
add("user-snippet", "a.lorem=5", `{
const x = 5;
const y = "const";
let b = "let";
var a = { xd: 5, yes: "let"};
a.lorem = 5
}`);
add("user-snippet", "only unique, no keyword reuse", `{
const x = 5;
const y = "zzzzz";
let b = "qqq";
var a = { xd: 5, yes: "qqq"};
a["lorem"] = 5
}`);
add("user-snippet", "reuse class (exact dictionary word)", `{
const x = 5;
const y = "class";
let b = "let";
var a = { xd: 5, yes: "let"};
a["class"] = 5
}`);
add("user-snippet", "reuse function (exact dictionary word + already a keyword in most programs)", `{
function x(){return 5}
const y = "function";
let b = "let";
var a = { xd: 5, yes: "let"};
a["function"] = 5
}`);

const five = ["const", "lorem", "class", "false", "value", "index", "async", "await", "throw", "yield", "xyzzy", "qwxkz"];
for (const word of five) {
  add("isolated-5", `bare ${JSON.stringify(word)}`, word, "Tiny stream. Header noise is large; still shows dictionary gravity.");
}

const six = ["return", "window", "length", "export", "import", "loremx", "xyzzyy", "module"];
for (const word of six) {
  add("isolated-6", `bare ${JSON.stringify(word)}`, word);
}

add("isolated-phrase", "type=\"text/javascript\"", 'type="text/javascript"');
add("isolated-phrase", "addEventListener", "addEventListener");
add("isolated-phrase", "function(){", "function(){");
add("isolated-phrase", "return;", "return;");
add("isolated-phrase", ".length", ".length");
add("isolated-phrase", "(typeof", "(typeof");
add("isolated-phrase", "){throw", "){throw");
add("isolated-phrase", "zzzzzzzzzzzzzzzzzzzzzz", "zzzzzzzzzzzzzzzzzzzzzz");

for (const word of ["const", "lorem", "class", "false", "function", "xyzzy"]) {
  add("js-string", `var x=${JSON.stringify(word)}`, `var x=${JSON.stringify(word)}`);
  add("js-key-quoted", `var a={${JSON.stringify(word)}:5}`, `var a={${JSON.stringify(word)}:5}`);
  add("js-key-bare", `var a={${word}:5}`, `var a={${word}:5}`, "Reserved words are legal unquoted keys.");
  add("js-dot", `a.${word}=5`, `a.${word}=5`);
  add("js-bracket", `a[${JSON.stringify(word)}]=5`, `a[${JSON.stringify(word)}]=5`);
}

add("reuse-vs-fresh", "keyword already present, string copies it", 'const x=1;const y="const"');
add("reuse-vs-fresh", "keyword present, unique string", 'const x=1;const y="lorem"');
add("reuse-vs-fresh", "no prior const, string is const", 'let x=1;let y="const"');
add("reuse-vs-fresh", "no prior const, string is lorem", 'let x=1;let y="lorem"');
add("reuse-vs-fresh", "function keyword + string function", 'function f(){return 1}var y="function"');
add("reuse-vs-fresh", "function keyword + unique 8", 'function f(){return 1}var y="zzzzzzzz"');
add("reuse-vs-fresh", "only string function, no keyword", 'var y="function"');
add("reuse-vs-fresh", "only unique 8", 'var y="zzzzzzzz"');

add("true-false-undefined", "true", "var a=true");
add("true-false-undefined", "!0", "var a=!0");
add("true-false-undefined", "false", "var a=false");
add("true-false-undefined", "!1", "var a=!1");
add("true-false-undefined", "undefined", "var a=undefined");
add("true-false-undefined", "void 0", "var a=void 0");
add("true-false-undefined", "null", "var a=null");

add("decl-keyword", "var x=5", "var x=5");
add("decl-keyword", "let x=5", "let x=5");
add("decl-keyword", "const x=5", "const x=5");

add("quotes", "double const", 'var y="const"');
add("quotes", "single const", "var y='const'");
add("quotes", "double class", 'var y="class"');
add("quotes", "single class", "var y='class'");
add("quotes", "double lorem", 'var y="lorem"');
add("quotes", "single lorem", "var y='lorem'");

add("identifier-alphabet", "short names", "function a(b,c){return b+c}");
add("identifier-alphabet", "dict words as params", "function length(value,index){return value+index}");
add("identifier-alphabet", "unique long names", "function qwxkz(qwxky,qwxkx){return qwxky+qwxkx}");
add("identifier-alphabet", "function as name (illegal as binding, used as string table)", 'function a(){return "function"}');
add("identifier-alphabet", "unique as string table", 'function a(){return "qwxkzzzz"}');

const repeatWords = ["const", "lorem", "class", "false", "function", "xyzzy", "value", "index"];
for (const word of repeatWords) {
  add("repeat-20", `20x ${JSON.stringify(word)} as object keys`, `var a={${Array.from({ length: 20 }, (_, i) => `${JSON.stringify(word)}:${i}`).join(",")}}`);
  add(
    "repeat-20-unique-keys",
    `20x unique vs one dict family (${word})`,
    `var a={${Array.from({ length: 20 }, (_, i) => `${JSON.stringify(word + i)}:${i}`).join(",")}}`,
  );
}

add(
  "repeat-user",
  "user const snippet x20",
  repeatBlock(userConst, 20),
  "Tiny files are header-dominated. Repeating the block makes the token choice visible.",
);
add("repeat-user", "user lorem snippet x20", repeatBlock(userLorem, 20));
add("repeat-user", "user const snippet x20 in unique padding", wrapUnique(repeatBlock(userConst, 20)));
add("repeat-user", "user lorem snippet x20 in unique padding", wrapUnique(repeatBlock(userLorem, 20)));
add("repeat-user", "user const snippet x20 in JS-like padding", wrapJsLike(repeatBlock(userConst, 20)));
add("repeat-user", "user lorem snippet x20 in JS-like padding", wrapJsLike(repeatBlock(userLorem, 20)));

add("html-in-js", "script type attr", 'var s=\'<script type="text/javascript">\'');
add("html-in-js", "unique same length", 'var s=\'<qwxkz type="zzzzzzzzzzzzzzzz">\'');
add("html-in-js", "class= attribute", 'var s=\'<div class="box">\'');
add("html-in-js", "unique attr", 'var s=\'<div qwxkz="box">\'');
add("html-in-js", "addEventListener", "element.addEventListener(\"click\",fn)");
add("html-in-js", "unique listener", "element.qwxkzzzzzzzzzz(\"click\",fn)");

add("dot-length", "a.length", "var n=a.length");
add("dot-length", "a.qwxkzz", "var n=a.qwxkzz");
add("dot-length", ".length bare", ".length");
add("dot-length", ".qwxkzz bare", ".qwxkzz");

add("transform-bait", "function(){", "function(){");
add("transform-bait", "function (){", "function (){");
add("transform-bait", "Function", "Function");
add("transform-bait", "FUNCTION", "FUNCTION");
add("transform-bait", "return;", "return;");
add("transform-bait", "return ", "return ");
add("transform-bait", "{return", "{return");
add("transform-bait", "){return", "){return");
add("transform-bait", "constant then const", 'var a="constant";var b="const"');
add("transform-bait", "loremipsum then lorem", 'var a="loremipsum";var b="lorem"');
add("transform-bait", "lets then let", 'var a="lets";var b="let"');
add("transform-bait", "xyzzy then let", 'var a="xyzzy";var b="let"');

add("pooling", "three copies of unique", 'var a="qwxkzunique";var b="qwxkzunique";var c="qwxkzunique"');
add("pooling", "three copies of class", 'var a="class";var b="class";var c="class"');
add("pooling", "three copies of const", 'var a="const";var b="const";var c="const"');
add("pooling", "pooled unique array", 'var p=["qwxkzunique"];var a=p[0];var b=p[0];var c=p[0]');
add("pooling", "pooled class array", 'var p=["class"];var a=p[0];var b=p[0];var c=p[0]');

add("context", "after equals", 'x="class"');
add("context", "after brace", '{"class"');
add("context", "after paren", '("class"');
add("context", "after unique", 'qwxkz="class"');
add("context", "after keyword", 'return "class"');

const probeTokens = [
  "const",
  "let",
  "var",
  "function",
  "return",
  "undefined",
  "prototype",
  "document",
  "window",
  "length",
  "class",
  "export",
  "import",
  "async",
  "await",
  "true",
  "false",
  "null",
  "this",
  "lorem",
  "xyzzy",
  "constructor",
  "addEventListener",
  "type=\"text/javascript\"",
  ".length",
  "(typeof",
  "){throw",
  "return;",
  "function(){",
];

function main() {
  const bin = ensureDictionary();
  const { words, byExact } = loadDictionaryWords(bin);
  const membership = {};
  for (const token of probeTokens) {
    membership[token] = {
      exact: byExact.has(token),
      hits: findDictionaryHits(token, words),
    };
  }

  const measured = cases.map((item) => ({
    ...item,
    sizes: sizes(item.text),
  }));

  const focus = {
    userConst: { text: userConst, sizes: sizesFull(userConst) },
    userLorem: { text: userLorem, sizes: sizesFull(userLorem) },
    isolatedClass: { text: "class", sizes: sizesFull("class") },
    isolatedConst: { text: "const", sizes: sizesFull("const") },
    isolatedLorem: { text: "lorem", sizes: sizesFull("lorem") },
    isolatedFunction: { text: "function", sizes: sizesFull("function") },
  };

  const interesting = words
    .filter((w) => {
      const lower = w.toLowerCase();
      return /function|return|undefined|prototype|document|window|length|javascript|script|class=|typeof|throw|const|addEvent|innerHTML|getElement|createElement|stylesheet|doctype|text\/|application\/|href|onclick|onload/.test(
        lower,
      );
    })
    .sort((a, b) => a.length - b.length || a.localeCompare(b));

  const report = {
    generatedAt: new Date().toISOString(),
    tools: {
      node: process.versions.node,
      nodeBrotli: process.versions.brotli,
      nodeZlib: process.versions.zlib,
      cliBrotli: execFileSync("brotli", ["-V"], { encoding: "utf8" }).trim(),
      pythonBrotli: execFileSync("python3", ["-c", "import brotli; print(brotli.__version__)"], {
        encoding: "utf8",
      }).trim(),
    },
    dictionary: {
      bytes: bin.length,
      words: words.length,
      minLen: 4,
      maxLen: 24,
      transforms: TRANSFORMS.length,
    },
    membership,
    interesting,
    affixes: Array.from({ length: 50 }, (_, i) => affix(i)),
    measured,
    focus,
  };

  const out = join(here, "brotli-mangle-lab.json");
  writeFileSync(out, JSON.stringify(report));
  console.log(`wrote ${out}`);
  console.log(`cases ${measured.length}`);
  console.log("user const", focus.userConst.sizes);
  console.log("user lorem", focus.userLorem.sizes);
  console.log(
    "membership",
    Object.fromEntries(
      Object.entries(membership).map(([k, v]) => [k, { exact: v.exact, hit: v.hits[0] || null }]),
    ),
  );
}

main();
