import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { build as esbuild } from "esbuild";
import { mkdirSync } from "node:fs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const buildRoot = join(labRoot, "build");
const compiled = join(labRoot, "ports/jquery/jquery-lilscript.raw.js");
const outFile = join(buildRoot, "jquery-lilscript-upstream-sel.js");
mkdirSync(buildRoot, { recursive: true });

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
}

run(compiler, [
  join(labRoot, "ports/jquery/entry.lil"),
  "--config",
  join(labRoot, "ports/jquery/lilscript.toml"),
  "--mode",
  "development",
  "--target",
  "js-module",
  "-o",
  compiled,
]);

await esbuild({
  absWorkingDir: join(labRoot, "ports/jquery"),
  entryPoints: [compiled],
  outfile: outFile,
  bundle: true,
  format: "esm",
  platform: "neutral",
  write: true,
});

const require = createRequire(import.meta.url);
const upstreamFactory = require("jquery");
const { JSDOM } = await import("jsdom");

const html = `<!doctype html><html><body>
<div id="qunit-fixture">
  <p id="firstp">See <a id="simon1" href="http://simon.incutio.com/archive/2003/03/25/#getElementsBySelector" rel="bookmark">this blog entry</a> for more information.</p>
  <p id="ap">Here are some links in a normal paragraph: <a id="google" href="http://www.google.com/" title="Google!">Google</a>,
  <a id="groups" href="http://groups.google.com/" class="GROUPS">Google Groups (Link)</a>.
  This link has <code><a href="http://smin" id="anchor1">class="blog"</a></code>:
  <a href="http://diveintomark.org/" class="blog" hreflang="en" id="mark">diveinto mark</a></p>
  <div id="foo">
    <p id="sndp">Everything inside the red border is inside a div with <code>id="foo"</code>.</p>
    <p lang="en" id="en">This is a normal link: <a id="yahoo" href="http://www.yahoo.com/" class="blog">Yahoo</a></p>
    <p id="sap">This link has <code><a href="#anchor1" id="anchor2">class="blog"</a></code>: <a href="http://simon.incutio.com/" class="blog link" id="simon">Simon Willison's Weblog</a></p>
  </div>
  <form id="form" action="formaction">
    <label for="action" id="label-for">Action:</label>
    <input type="text" name="action" value="Test" id="action" />
    <input type="text" name="text" id="text1" />
    <input type="hidden" name="hidden" id="hidden1" />
    <select id="select1" name="select1">
      <option id="option1a" value="">Nothing</option>
      <option id="option1b" value="1">1</option>
      <option id="option1c" value="2">2</option>
      <option id="option1d" value="3">3</option>
    </select>
  </form>
  <div id="siblingTest">
    <em>x</em><em>y</em><span>z</span>
  </div>
  <ol id="listWithTabIndex" tabindex="5">
    <li id="list1">one</li>
    <li id="list2">two</li>
    <li id="list3">three</li>
    <li id="list4">four</li>
  </ol>
</div>
</body></html>`;

const dom = new JSDOM(html, { pretendToBeVisual: true });
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const upstream = upstreamFactory(dom.window);
const lil = await import(outFile);
const $u = upstream;
const $l = lil.jQuery;

function ids($col) {
  return $col.map(function () { return this.id; }).get();
}

const cases = [
  ["#qunit-fixture p", (c) => ids(c)],
  ["p:has(a)", (c) => ids(c)],
  ["a:contains(Google)", (c) => ids(c)],
  ["a:contains('Google Groups')", (c) => ids(c)],
  ["#listWithTabIndex li:eq(2) ~ li", (c) => ids(c)],
  ["#siblingTest > em:contains('x') + em ~ span", (c) => ids(c)],
  ["#qunit-fixture p:has(:contains(mark)):has(code)", (c) => ids(c)],
  ["#form select:has(option:first-child:contains('o'))", (c) => ids(c)],
  [":input", (c) => ids(c)],
  ["#qunit-fixture a[rel='bookmark']", (c) => ids(c)],
  ["#qunit-fixture p:not(#firstp)", (c) => ids(c)],
  ["#foo > p", (c) => ids(c)],
];

let failed = 0;
for (const [selector, extract] of cases) {
  const u = extract($u(selector));
  const l = extract($l(selector));
  try {
    assert.deepEqual(l, u, selector);
    console.log(`OK   ${selector} -> ${JSON.stringify(l)}`);
  } catch {
    failed += 1;
    console.log(`FAIL ${selector}\n  upstream: ${JSON.stringify(u)}\n  lil:      ${JSON.stringify(l)}`);
  }
}

if (failed) {
  console.log(`\n${failed} upstream-selector case(s) failed`);
  process.exit(1);
}
console.log("\njquery-upstream-selector:all:ok");
