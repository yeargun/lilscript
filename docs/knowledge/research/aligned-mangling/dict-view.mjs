#!/usr/bin/env node
/* The hardcoded library, from a JavaScript emitter's point of view.

   Prints what is actually in Brotli's 122,784-byte static dictionary that a
   JS artifact could ever hit, and what our own streams hit in practice. */
import { loadEngine } from "../brotli-machine/engine.mjs";
import { CORPORA, readCorpus, census } from "./census.mjs";

const BM = loadEngine();
const dict = BM.dictionary();

const IDENT = /^[A-Za-z_$][A-Za-z0-9_$]*$/;
const JS_KEYWORDS = new Set(["function", "return", "var", "let", "const", "if", "else", "for", "while",
  "typeof", "instanceof", "new", "this", "null", "true", "false", "undefined", "class", "extends",
  "import", "export", "default", "try", "catch", "finally", "throw", "switch", "case", "break",
  "continue", "delete", "void", "yield", "await", "async", "static", "get", "set", "in", "of"]);
const WEB = new Set(["document", "window", "prototype", "constructor", "addEventListener", "length",
  "value", "name", "type", "data", "index", "callback", "element", "attribute", "className",
  "innerHTML", "getElementById", "createElement", "appendChild", "parentNode", "nodeType",
  "childNodes", "style", "display", "position", "target", "event", "options", "onclick", "script",
  "object", "array", "string", "number", "boolean", "error", "message", "result", "toString",
  "valueOf", "hasOwnProperty", "Object", "Array", "String", "Number", "Boolean", "Function",
  "Math", "JSON", "Promise", "Symbol", "Map", "Set", "push", "pop", "slice", "splice", "concat",
  "filter", "map", "reduce", "forEach", "indexOf", "charAt", "substring", "replace", "split", "join"]);

function main() {
  console.log("## shape\n");
  console.log("| length | words | first | last |");
  console.log("|---:|---:|---|---|");
  let total = 0;
  for (let len = 4; len <= 24; len++) {
    const n = dict.countFor(len);
    if (!n) continue;
    total += n;
    console.log(`| ${len} | ${n} | \`${dict.wordText(len, 0)}\` | \`${dict.wordText(len, n - 1)}\` |`);
  }
  console.log(`\n${total} words, ${dict.bytes.length} bytes, ${dict.transforms.length} transforms.\n`);

  /* Which JS-shaped strings are in there at all. */
  const identifiers = [];
  const keywords = [];
  const web = [];
  for (let len = 4; len <= 24; len++) {
    const n = dict.countFor(len);
    for (let i = 0; i < n; i++) {
      const word = dict.wordText(len, i);
      if (!IDENT.test(word)) continue;
      identifiers.push(word);
      if (JS_KEYWORDS.has(word)) keywords.push(word);
      if (WEB.has(word)) web.push(word);
    }
  }
  console.log(`## JavaScript-shaped entries\n`);
  console.log(`${identifiers.length} of the ${total} words are legal identifiers standing alone.\n`);

  /* The useful question is not "is the bare word in there" but "is there a
     word plus transform that spells exactly what I have to emit". */
  const probes = [
    "function", "function(", "function ", "return", "return ", ");return ", "typeof ", "typeof",
    "var ", "let ", "const ", "new ", "this.", ".length", "length", ".prototype", "prototype",
    ".call(", ".apply(", "undefined", "null", "true", "false", "document", "window", ".document",
    "Object", "Object.", "Array", "Math.", ".push(", ".indexOf(", ".toString", "constructor",
    "addEventListener", "createElement", "getElementById", "parentNode", "nodeType", "childNodes",
    "className", "innerHTML", "style.", "value", "target", "options", "callback", "element",
    "for(var ", "if(", "else{", "}else{", "){return ", "=function(", ".prototype.", "JSON.",
    "Promise", "Symbol", "async ", "await ", "=>{", "export ", "import ",
  ];
  console.log("| token | served by the dictionary? | how |");
  console.log("|---|---|---|");
  let served = 0;
  for (const probe of probes) {
    const hits = dict.matchesAt(probe, 0, {}).filter((h) => h.produced === probe);
    if (hits.length) {
      served++;
      const h = hits[0];
      console.log(`| \`${probe.replace(/\|/g, "\\|")}\` | yes | word ${h.wordIndex} of length ${h.len} \`${dict.wordText(h.len, h.wordIndex)}\`, transform ${h.transform} (${dict.describeTransform(h.transform)}) |`);
    } else {
      const partial = dict.matchesAt(probe, 0, {})[0];
      console.log(`| \`${probe.replace(/\|/g, "\\|")}\` | no | ${partial ? `only the first ${partial.matched} bytes, as \`${partial.produced}\`` : "nothing"} |`);
    }
  }
  console.log(`\n${served} of ${probes.length} probed spellings are exactly one dictionary reference.\n`);

  /* The transforms that matter to code, not prose. */
  console.log("## transforms a JS emitter can reach\n");
  console.log("| # | transform | `function` becomes |");
  console.log("|---:|---|---|");
  const interesting = [];
  for (let t = 0; t < dict.transforms.length; t++) {
    const { prefix, suffix, typeName } = dict.transformParts(t);
    const codeish = /[.(){}[\];:=<>/"'\n\t,]/.test(prefix + suffix);
    if (codeish || (prefix === "" && suffix === "" && typeName !== "IDENTITY")) interesting.push(t);
  }
  for (const t of interesting.slice(0, 40)) {
    console.log(`| ${t} | ${dict.describeTransform(t)} | \`${dict.applyTransform("function", t).replace(/\n/g, "\\n").replace(/\t/g, "\\t")}\` |`);
  }
  console.log(`\n${interesting.length} of ${dict.transforms.length} transforms carry code punctuation.\n`);

  /* What our own artifacts actually pull out of it. */
  console.log("## what our streams actually use\n");
  console.log("| corpus | dictionary refs | bytes from the dictionary | share of output | distinct entries | reused entries |");
  console.log("|---|---:|---:|---:|---:|---:|");
  const used = new Map();
  for (const id of Object.keys(CORPORA)) {
    const text = readCorpus(id);
    const c = census(id, text);
    for (const [word, n] of c.topDictWords) used.set(word, (used.get(word) || 0) + n);
    const reused = c.dictRefs - c.distinctDictEntries;
    console.log(`| ${id} | ${c.dictRefs} | ${c.dictBytes} | ${((c.dictBytes / c.raw) * 100).toFixed(2)}% | ${c.distinctDictEntries} | ${reused} |`);
  }
  console.log(`\nMost-used entries across the corpora: ${[...used.entries()].sort((a, b) => b[1] - a[1]).slice(0, 24).map(([w, n]) => `\`${JSON.stringify(w).slice(1, -1)}\`×${n}`).join(" ")}\n`);
}

main();
