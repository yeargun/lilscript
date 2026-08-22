#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const here = fileURLToPath(new URL(".", import.meta.url));
const report = JSON.parse(readFileSync(join(here, "brotli-mangle-lab.json"), "utf8"));

const slim = {
  generatedAt: report.generatedAt,
  tools: report.tools,
  dictionary: report.dictionary,
  membership: Object.fromEntries(
    Object.entries(report.membership).map(([token, info]) => [
      token,
      { exact: info.exact, hits: info.hits.slice(0, 3) },
    ]),
  ),
  affixes: report.affixes,
  interesting: report.interesting,
  measured: report.measured.map((item) => ({
    group: item.group,
    name: item.name,
    text: item.text.length > 400 ? `${item.text.slice(0, 220)}\n… [${item.text.length} bytes]` : item.text,
    note: item.note,
    sizes: item.sizes,
  })),
  focus: {
    userConst: report.focus.userConst,
    userLorem: report.focus.userLorem,
  },
};

const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Brotli mangling lab</title>
<style>
:root {
  --bg: #12110f;
  --bg2: #1b1916;
  --bg3: #24211c;
  --ink: #ece7dc;
  --muted: #9a9283;
  --line: #3a342b;
  --accent: #d8a15a;
  --win: #7dba7a;
  --lose: #d37a6a;
  --mono: "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  --sans: "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--ink);
  font: 17px/1.5 var(--sans);
}
a { color: var(--accent); }
code, pre, .mono, table { font-family: var(--mono); }
header {
  padding: 48px 28px 28px;
  border-bottom: 1px solid var(--line);
  background: var(--bg2);
}
header p { max-width: 72ch; color: var(--muted); }
nav {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 18px;
  padding: 14px 28px;
  border-bottom: 1px solid var(--line);
  position: sticky;
  top: 0;
  background: #12110fef;
  z-index: 2;
  font-size: 13px;
  font-family: var(--mono);
}
nav a { text-decoration: none; }
main { padding: 28px; max-width: 1180px; }
section { margin: 0 0 56px; }
h1 { font-size: 34px; line-height: 1.15; margin: 0 0 12px; }
h2 { font-size: 24px; margin: 0 0 12px; }
h3 { font-size: 18px; margin: 28px 0 8px; }
p, li { max-width: 74ch; }
.lede { font-size: 20px; max-width: 68ch; }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.card {
  background: var(--bg2);
  border: 1px solid var(--line);
  padding: 16px 16px 12px;
}
.card h3 { margin-top: 0; }
pre {
  background: #0c0b0a;
  border: 1px solid var(--line);
  padding: 12px 14px;
  overflow: auto;
  font-size: 13px;
  line-height: 1.45;
}
.kvs { display: grid; grid-template-columns: 160px 1fr; gap: 4px 12px; font-family: var(--mono); font-size: 13px; }
.kvs b { color: var(--muted); font-weight: 400; }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th, td { border-bottom: 1px solid var(--line); padding: 6px 8px; text-align: left; vertical-align: top; }
th { color: var(--muted); font-weight: 500; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.win { color: var(--win); }
.lose { color: var(--lose); }
.note { color: var(--muted); font-size: 13px; }
.pill {
  display: inline-block;
  border: 1px solid var(--line);
  padding: 1px 7px;
  font-family: var(--mono);
  font-size: 11px;
  color: var(--muted);
}
.pill.in { color: var(--win); border-color: #355334; }
.pill.out { color: var(--lose); border-color: #5a322c; }
input, select, button {
  background: var(--bg3);
  color: var(--ink);
  border: 1px solid var(--line);
  padding: 7px 9px;
  font: 13px var(--mono);
}
.toolbar { display: flex; flex-wrap: wrap; gap: 8px; margin: 0 0 12px; }
svg.chart { width: 100%; height: 220px; background: #0c0b0a; border: 1px solid var(--line); }
.callout {
  border-left: 3px solid var(--accent);
  padding: 2px 0 2px 14px;
  margin: 16px 0;
}
.museum { display: grid; gap: 14px; }
.museum article { background: var(--bg2); border: 1px solid var(--line); padding: 14px 16px; }
details { margin: 10px 0; }
summary { cursor: pointer; color: var(--accent); }
.tiny { font-size: 12px; color: var(--muted); }
</style>
</head>
<body>
<header>
  <div class="pill">measured 2026-08-19 · brotli 1.1.0 · gzip 9 · RFC 7932 dictionary</div>
  <h1>Brotli does have hardcoded web keys. That is not the whole hack.</h1>
  <p class="lede">Every Brotli decoder ships a frozen 122,784-byte dictionary of 13,504 web-ish strings, plus 121 transforms. Your <code>a["const"]</code> vs <code>a["lorem"]</code> idea is real. On this machine the win is mostly <em>copying a token already in the file</em>, not the static dictionary. Quality 11 can hide a gap that gzip and quality 5 still see.</p>
</header>
<nav>
  <a href="#how">How it works</a>
  <a href="#dict">Dictionary</a>
  <a href="#your-hack">Your snippets</a>
  <a href="#quirks">Quirk museum</a>
  <a href="#versions">Quality / mode / version</a>
  <a href="#playbook">Mangling playbook</a>
  <a href="#lab">Full lab</a>
  <a href="#rerun">Re-run</a>
</nav>
<main>
<section id="how">
  <h2>How Brotli actually emits bytes</h2>
  <p>Brotli is not “gzip with a better window.” A substring can be paid for in three different ways, and the encoder picks whichever costs fewer bits <em>in this file, at this quality, with this context model</em>.</p>
  <div class="grid">
    <div class="card">
      <h3>1. Literal Huffman</h3>
      <p>Raw bytes, but the code for <code>e</code> is cheaper than <code>q</code> if <code>e</code> is common nearby. Previous 1–2 bytes pick which literal tree is used. <code>return "</code> and <code>qwxkz="</code> do not cost the same.</p>
    </div>
    <div class="card">
      <h3>2. LZ77 copy from this file</h3>
      <p>After <code>const x = 5</code>, the later string <code>"const"</code> can copy those five letters. Gzip can do this too. This is the mechanism your snippet actually hits. Minimum useful copy is short; distance and length still cost bits.</p>
    </div>
    <div class="card">
      <h3>3. Static dictionary + transform</h3>
      <p>The decoder pretends 13,504 words already sit behind the sliding window. The encoder may emit “copy word #N after transform #T” without those bytes ever appearing in the file. Gzip has no equivalent.</p>
    </div>
    <div class="card">
      <h3>4. Block / context / quality search</h3>
      <p>Quality changes how hard the encoder looks. Mode (generic / text / font) changes context maps. Quality is not monotonic: q11 can lose to q5 on tiny JS. LilScript scores official C 1.1.0, generic, q11, <code>lgwin=22</code>.</p>
    </div>
  </div>
  <div class="callout">
    <p>Dictionary matches are whole transformed words. You cannot start in the middle of <code>function</code> and stop at <code>func</code> unless a transform (omit-last / omit-first) produces exactly that string. Adjacent dictionary words do not glue together.</p>
  </div>
</section>

<section id="dict">
  <h2>Yes: web keys are hardcoded. They are frozen.</h2>
  <div class="kvs">
    <b>spec</b><span>RFC 7932 Appendix A, 2016. Same bytes in brotli 0.6 through 1.2.</span>
    <b>size</b><span>122,784 bytes of words · 13,504 entries · lengths 4–24</span>
    <b>transforms</b><span>121: identity, omit first/last 1–9, uppercase first/all, plus packed prefixes/suffixes</span>
    <b>trained on</b><span>2013–2015 web crawl: HTML, JS, CSS, HTTP, English, some other languages</span>
    <b>not in the ROM</b><span><code>const</code> exact, <code>let</code>, <code>var</code>, <code>lorem</code>, <code>await</code>, <code>constructor</code>, <code>export</code> exact</span>
    <b>in the ROM</b><span><code>function</code>, <code>function(){</code>, <code>return;</code>, <code>undefined</code>, <code>prototype</code>, <code>class</code>, <code>false</code>, <code>addEventListener</code>, <code>type="text/javascript"</code></span>
  </div>
  <h3>The omit-last trap</h3>
  <p><code>const</code> is not a dictionary word. <code>constant</code> is. Transform 23 is “omit last 3,” so the encoder <em>may</em> emit <code>const</code> as a dictionary reference. Same story: <code>let</code> ← <code>lets</code> omit-last-1, <code>var</code> ← <code>vary</code>/<code>vars</code> omit-last-1, <code>export</code> ← <code>exports</code>. The encoder does not have to take that path. A 5-byte unique word and a 5-byte transform candidate often tie on a tiny stream because the 4–6 byte Brotli header dominates.</p>
  <h3>Transforms that look like HTML / JS punctuation</h3>
  <p>Prefixes and suffixes are not generic. They are web glue: space, <code>="</code>, <code>='</code>, <code>"</code>, <code>'</code>, <code>.</code>, <code>.com/</code>, <code>ing </code>, <code> the </code>, newline. That is why <code>.length</code> is a first-class dictionary hit (<code>"." + length</code>) and why <code>function (){</code> with a space loses the exact word <code>function(){</code>.</p>
  <div id="membership"></div>
  <h3>Dictionary museum (JS / HTML-ish words actually in the ROM)</h3>
  <div class="toolbar">
    <input id="dictFilter" placeholder="filter dictionary words" size="40">
    <span class="tiny" id="dictCount"></span>
  </div>
  <div id="dictList" class="tiny" style="max-height:240px;overflow:auto;border:1px solid var(--line);padding:10px;background:#0c0b0a;white-space:pre-wrap"></div>
</section>

<section id="your-hack">
  <h2>Your two snippets, measured</h2>
  <p>Both files are 95 raw bytes. Same whitespace. The only edit is the last key.</p>
  <div class="grid">
    <div class="card">
      <h3>a["const"] = 5</h3>
      <pre>${escapeHtml(report.focus.userConst.text)}</pre>
      <p class="mono">gzip9 ${report.focus.userConst.sizes.gzip9} · br q5 ${report.focus.userConst.sizes.br5g} · br q11 ${report.focus.userConst.sizes.br11g} · cli/node/py q11 all ${report.focus.userConst.sizes.brCli11}</p>
    </div>
    <div class="card">
      <h3>a["lorem"] = 5</h3>
      <pre>${escapeHtml(report.focus.userLorem.text)}</pre>
      <p class="mono">gzip9 ${report.focus.userLorem.sizes.gzip9} · br q5 ${report.focus.userLorem.sizes.br5g} · br q11 ${report.focus.userLorem.sizes.br11g} · cli/node/py q11 all ${report.focus.userLorem.sizes.brCli11}</p>
    </div>
  </div>
  <div class="callout">
    <p><strong>q11 ties at 83.</strong> gzip still prefers const (91 vs 96). q5 prefers const (77 vs 81). Repeat the block 20 times and q11 finally splits: 87 vs 90. Surround it with unique JS and the gap opens to 300 vs 309. The hack is real; a 95-byte file is too small for q11 to care.</p>
  </div>
  <svg class="chart" id="qualityChart" viewBox="0 0 640 220" role="img" aria-label="Brotli quality 0-11 for the two snippets"></svg>
  <p class="note">Quality is not monotonic. On the const snippet, q5–q8 are 77 and q11 is 83. LilScript ranks q11 generic anyway, because that is what you want to serve. Do not tune mangling against q5 if production is q11.</p>
  <h3>Same family, weirder spellings</h3>
  <div id="userFamily"></div>
</section>

<section id="quirks" class="museum">
  <h2>Quirk museum</h2>
  <article>
    <h3>1. a.lorem is much worse than a["lorem"]</h3>
    <p>In the full snippet, <code>a.const=5</code> is the q11 winner at 82. <code>a.lorem=5</code> is 92 — worse than adding quotes. Quotes and brackets already exist in the file, so <code>a["lorem"]</code> reuses syntax. A fresh identifier <code>lorem</code> after a dot does not.</p>
  </article>
  <article>
    <h3>2. Blindly picking a dictionary word can lose</h3>
    <p>Replacing the reused <code>"const"</code> with the exact dictionary word <code>"class"</code> made q11 <em>worse</em> (91 vs 83). You spent a new token and lost the LZ77 copy of a keyword that already appeared twice. Dictionary is a first-occurrence discount. It is not a reason to throw away a copy you already have.</p>
  </article>
  <article>
    <h3>3. Isolated 5-letter words all compress to 9 bytes</h3>
    <p><code>const</code>, <code>class</code>, <code>lorem</code>, <code>xyzzy</code>, <code>false</code>: all raw 5, gzip 25, brotli 9. Do not design a mangler from isolated-token tests. The stream header is larger than the signal.</p>
  </article>
  <article>
    <h3>4. Repeated keys: function beats shorter unique words</h3>
    <p>Twenty identical object keys, q11: <code>function</code> 63 (raw 277), <code>class</code>/<code>value</code>/<code>index</code> 65, <code>false</code> 66, <code>xyzzy</code> 69, <code>const</code> 71, <code>lorem</code> 72. A longer dictionary word, used as a repeated property, can beat a shorter unique word. Twenty <em>unique</em> keys in the same family cost ~88–94.</p>
  </article>
  <article>
    <h3>5. Short identifiers still win as bindings</h3>
    <p><code>function a(b,c){return b+c}</code> is 31. The same shape with <code>length</code>/<code>value</code>/<code>index</code> is 32 despite 21 extra raw bytes. Unique long names are 40. Use dictionary words for <em>one-off strings and keys</em>, not for hot locals. Hot locals want <code>a</code>, <code>b</code>, <code>c</code> and LZ77 of one-byte copies.</p>
  </article>
  <article>
    <h3>6. Keep function(){ glued</h3>
    <p><code>function(){</code> is itself a dictionary word: 11 raw, 11 brotli. Insert a space and it becomes 13. <code>){return</code> is 10 for 8 raw bytes. Do not pretty-print the shapes the ROM already knows.</p>
  </article>
  <article>
    <h3>7. HTML phrases inside JS are almost free</h3>
    <p><code>&lt;script type="text/javascript"&gt;</code> is 29 brotli vs 43 for a same-length unique string. <code>addEventListener</code> is 30 vs 34 for a unique method name. If a closed-world compiler must emit a host name, prefer the real DOM/HTML token over a cute alias.</p>
  </article>
  <article>
    <h3>8. !0 still beats true</h3>
    <p><code>true</code> and <code>false</code> are dictionary words. <code>var a=!0</code> is still 12 vs 14, <code>!1</code> is 12 vs 15, <code>void 0</code> is 16 vs 19 for <code>undefined</code>. The rewrite is two to three raw bytes shorter; Brotli does not give the long forms back on a tiny program.</p>
  </article>
  <article>
    <h3>9. Pooling can fight the codec</h3>
    <p>Three copies of <code>"class"</code> compress to 30. Wrapping them in <code>p=["class"];a=p[0]…</code> becomes 52. Three copies of <code>"const"</code> at q11 stay 41 — the encoder fails to reuse them — while q5 gets 31. LilScript already treats pooling as a scored candidate, not a free win.</p>
  </article>
  <article>
    <h3>10. await is missing; async is not</h3>
    <p>The crawl predates widespread <code>await</code>. <code>async</code> is in. <code>export</code> is only <code>exports</code> minus one letter. <code>constructor</code> is out; <code>prototype</code> is in. Prefer <code>prototype</code> as a dummy key over <code>constructor</code> if you need a long legal identifier.</p>
  </article>
  <article>
    <h3>11. Context after return is cheaper than after qwxkz</h3>
    <p><code>return "class"</code> is 15. <code>qwxkz="class"</code> is 17. Same dictionary word, different previous bytes, different literal context tree.</p>
  </article>
  <article>
    <h3>12. var / let / const as declarations</h3>
    <p>On a one-line program they cost their raw length: 11 / 11 / 13. <code>let</code> and <code>var</code> are three bytes, below the dictionary minimum of four, so they only win as LZ77 copies or via omit-last of <code>lets</code>/<code>vary</code>. LilScript already scores <code>let</code> vs <code>var</code> on the complete artifact for this reason.</p>
  </article>
</section>

<section id="versions">
  <h2>Versions, modes, qualities</h2>
  <p>The <em>format</em> dictionary never changes. If a decoder cannot find <code>function(){</code> in ROM, it is not Brotli. What changes across “versions” is the encoder’s search: match finding, context modeling, block splitting, when to spend a dictionary reference.</p>
  <div class="kvs">
    <b>this machine</b><span>CLI brotli 1.1.0 · Node 20.12 brotli 1.1.0 · Python brotli 1.1.0</span>
    <b>q11 generic</b><span>const snippet 83 on CLI, Node, and Python. Same bitstream length.</span>
    <b>MODE_TEXT</b><span>Tied GENERIC on every JS snippet in this lab. Do not assume TEXT helps JS.</span>
    <b>MODE_FONT</b><span>Sometimes different (class-reuse snippet 81 vs 91). Irrelevant for JS delivery.</span>
    <b>no context model</b><span>const snippet 80, lorem 83. The context model can <em>hurt</em> a tiny file at q11.</span>
    <b>LilScript</b><span>Bundled Google C 1.1.0, generic, q11, lgwin 22. Node zlib is diagnostic only.</span>
    <b>CDNs</b><span>Often q4–q6 for dynamic, q11 for static. A q5-optimal mangle can lose at q11.</span>
    <b>Cloudflare reduced dict</b><span>Encoder-side subset for speed. Stream is still RFC 7932. Decoder ROM is full.</span>
    <b>RFC 9842</b><span>Compression Dictionary Transport is a <em>different</em> dictionary, negotiated per site. Not this ROM.</span>
  </div>
  <p>Older encoders (0.5, 0.6, early 1.0) find fewer dictionary/transform matches and split blocks worse. They still decode the same dictionary. If you ever compare “brotli versions,” say whether you changed the encoder, the quality, the mode, or the window. Those are four knobs.</p>
</section>

<section id="playbook">
  <h2>Mangling playbook for a Brotli-first compiler</h2>
  <ol>
    <li>Reuse a token already in the file before inventing a new one. Keyword → string → property is the cheap direction. This is your hack, and gzip sees it even when q11 ties.</li>
    <li>For a <em>one-off</em> string or key with no prior copy, prefer an exact ROM word: <code>function</code>, <code>class</code>, <code>false</code>, <code>value</code>, <code>index</code>, <code>return</code>, <code>undefined</code>, <code>prototype</code>, <code>length</code>, <code>document</code>. Do not pick <code>lorem</code>, <code>xyzzy</code>, <code>await</code>, <code>constructor</code>.</li>
    <li>Do not replace an existing LZ77 copy with a “better” dictionary word. The class-for-const swap lost.</li>
    <li>Hot locals stay one byte. Dictionary words as parameter names almost tied short names once, then lose as soon as the names stop being exact ROM words.</li>
    <li>Prefer <code>a.const</code> over <code>a["const"]</code> when <code>const</code> is already a keyword. Prefer <code>a["unique"]</code> over <code>a.unique</code> when quotes already exist and the identifier does not.</li>
    <li>Keep <code>function(){</code>, <code>return;</code>, <code>){return</code>, <code>.length</code> intact. Spaces and pretty-print break exact words.</li>
    <li>Host / HTML / DOM names: emit the real token. Aliasing <code>addEventListener</code> to a unique identifier is a transfer tax.</li>
    <li>Keep <code>!0</code> / <code>!1</code> / <code>void 0</code> in the candidate set. Dictionary membership does not retire them.</li>
    <li>Score pooling. Copies of a ROM word are often cheaper than an array table.</li>
    <li>Never rank a candidate on an isolated token or on a 95-byte file alone. Repeat it, pad it, and score the complete artifact under the codec you serve.</li>
    <li><code>const</code> is a second-class dictionary citizen (omit-last of <code>constant</code>). Treat it as LZ77 bait, not as a ROM word.</li>
    <li>Declaration spelling (<code>var</code>/<code>let</code>/<code>const</code>) belongs in the beam. The surrounding file decides, not a global rule.</li>
  </ol>
</section>

<section id="lab">
  <h2>Full lab (148 cases)</h2>
  <p class="note">Bytes are Node zlib brotli 1.1.0 unless noted. Smaller is better. Green is the best brotli q11 in the visible group.</p>
  <div class="toolbar">
    <select id="groupFilter"></select>
    <input id="caseFilter" placeholder="filter name or source" size="40">
    <label class="tiny"><input type="checkbox" id="sortBrotli"> sort by br q11</label>
  </div>
  <div style="overflow:auto"><table id="labTable"></table></div>
</section>

<section id="rerun">
  <h2>Re-run from CLI</h2>
  <pre>node docs/knowledge/research/brotli-mangle-lab.mjs
node docs/knowledge/research/render-brotli-mangle-lab.mjs

# one pair, the way this page was checked:
printf '%s' '{
const x = 5;
const y = "const";
let b = "let";
var a = { xd: 5, yes: "let"};
a["const"] = 5
}' | brotli -q 11 | wc -c

python3 -c 'import brotli,sys; print(len(brotli.compress(sys.stdin.buffer.read(), quality=11)))'
</pre>
  <p class="note">Generated ${report.generatedAt}. Tools: Node ${report.tools.node} / brotli ${report.tools.nodeBrotli} / zlib ${report.tools.nodeZlib}; ${report.tools.cliBrotli}; Python brotli ${report.tools.pythonBrotli}.</p>
</section>
</main>
<script type="application/json" id="lab-data">${JSON.stringify(slim)}</script>
<script>
const DATA = JSON.parse(document.getElementById("lab-data").textContent);

function renderMembership() {
  const rows = Object.entries(DATA.membership).map(([token, info]) => {
    const hit = info.hits[0];
    const how = info.exact ? "exact word" : hit ? hit.word + " · " + hit.transform : "no path";
    return \`<tr><td><code>\${escape(token)}</code></td><td>\${info.exact ? '<span class="pill in">exact</span>' : hit ? '<span class="pill">transform</span>' : '<span class="pill out">absent</span>'}</td><td class="tiny">\${escape(how)}</td></tr>\`;
  });
  document.getElementById("membership").innerHTML = \`<table><thead><tr><th>token</th><th>ROM</th><th>how Brotli can still emit it</th></tr></thead><tbody>\${rows.join("")}</tbody></table>\`;
}

function renderDict(filter = "") {
  const q = filter.toLowerCase();
  const words = DATA.interesting.filter((w) => !q || w.toLowerCase().includes(q));
  document.getElementById("dictCount").textContent = words.length + " / " + DATA.interesting.length + " JS-ish words";
  document.getElementById("dictList").textContent = words.map((w) => JSON.stringify(w)).join("  ");
}

function renderUserFamily() {
  const rows = DATA.measured.filter((c) => c.group === "user-snippet");
  const best = Math.min(...rows.map((r) => r.sizes.br11g));
  document.getElementById("userFamily").innerHTML = tableFor(rows, best);
}

function tableFor(rows, bestBr) {
  return \`<table><thead><tr><th>case</th><th class="num">raw</th><th class="num">gzip9</th><th class="num">br q5</th><th class="num">br q11</th><th>source</th></tr></thead><tbody>\${rows.map((r) => {
    const br = r.sizes.br11g;
    const cls = br === bestBr ? "win" : "";
    return \`<tr><td>\${escape(r.name)}</td><td class="num">\${r.sizes.raw}</td><td class="num">\${r.sizes.gzip9}</td><td class="num">\${r.sizes.br5g}</td><td class="num \${cls}">\${br}</td><td><pre style="margin:0;border:0;background:transparent;padding:0">\${escape(r.text)}</pre></td></tr>\`;
  }).join("")}</tbody></table>\`;
}

function renderLab() {
  const group = document.getElementById("groupFilter").value;
  const q = document.getElementById("caseFilter").value.toLowerCase();
  let rows = DATA.measured.filter((r) => (group === "all" || r.group === group) && (!q || (r.name + r.text).toLowerCase().includes(q)));
  if (document.getElementById("sortBrotli").checked) rows = [...rows].sort((a, b) => a.sizes.br11g - b.sizes.br11g || a.sizes.raw - b.sizes.raw);
  const best = rows.length ? Math.min(...rows.map((r) => r.sizes.br11g)) : -1;
  document.getElementById("labTable").innerHTML = \`<thead><tr><th>group</th><th>case</th><th class="num">raw</th><th class="num">gzip9</th><th class="num">q5</th><th class="num">q11 g</th><th class="num">q11 t</th><th>source</th></tr></thead><tbody>\${rows.map((r) => \`<tr><td class="tiny">\${escape(r.group)}</td><td>\${escape(r.name)}</td><td class="num">\${r.sizes.raw}</td><td class="num">\${r.sizes.gzip9}</td><td class="num">\${r.sizes.br5g}</td><td class="num \${r.sizes.br11g === best ? "win" : ""}">\${r.sizes.br11g}</td><td class="num">\${r.sizes.br11t}</td><td><pre style="margin:0;border:0;background:transparent;padding:0;max-width:420px">\${escape(r.text)}</pre></td></tr>\`).join("")}</tbody>\`;
}

function drawQuality() {
  const a = DATA.focus.userConst.sizes.qualities;
  const b = DATA.focus.userLorem.sizes.qualities;
  const qs = Object.keys(a);
  const vals = qs.flatMap((q) => [a[q].generic, b[q].generic]);
  const min = Math.min(...vals) - 4;
  const max = Math.max(...vals) + 4;
  const x = (i) => 40 + (i * 540) / (qs.length - 1);
  const y = (v) => 190 - ((v - min) * 160) / (max - min);
  const path = (series) => qs.map((q, i) => (i ? "L" : "M") + x(i) + " " + y(series[q].generic)).join(" ");
  const ticks = qs.map((q, i) => \`<text x="\${x(i)}" y="208" fill="#9a9283" font-size="11" text-anchor="middle">\${q}</text>\`);
  document.getElementById("qualityChart").innerHTML = \`
    <text x="12" y="18" fill="#9a9283" font-size="11">brotli bytes</text>
    <path d="\${path(a)}" fill="none" stroke="#7dba7a" stroke-width="2"/>
    <path d="\${path(b)}" fill="none" stroke="#d37a6a" stroke-width="2"/>
    \${qs.map((q, i) => \`<circle cx="\${x(i)}" cy="\${y(a[q].generic)}" r="3" fill="#7dba7a"/><circle cx="\${x(i)}" cy="\${y(b[q].generic)}" r="3" fill="#d37a6a"/>\`).join("")}
    \${ticks.join("")}
    <text x="480" y="24" fill="#7dba7a" font-size="12">a["const"]</text>
    <text x="560" y="24" fill="#d37a6a" font-size="12">a["lorem"]</text>
  \`;
}

function escape(s) {
  return String(s).replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]));
}

const groups = ["all", ...new Set(DATA.measured.map((r) => r.group))];
document.getElementById("groupFilter").innerHTML = groups.map((g) => \`<option>\${g}</option>\`).join("");
document.getElementById("dictFilter").addEventListener("input", (e) => renderDict(e.target.value));
document.getElementById("groupFilter").addEventListener("change", renderLab);
document.getElementById("caseFilter").addEventListener("input", renderLab);
document.getElementById("sortBrotli").addEventListener("change", renderLab);
renderMembership();
renderDict();
renderUserFamily();
renderLab();
drawQuality();
</script>
</body>
</html>
`;

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]));
}

const out = join(here, "brotli-mangle-lab.html");
writeFileSync(out, html);
console.log("wrote", out, html.length);
