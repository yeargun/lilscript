import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import test from "node:test";
const read = name => readFile(new URL(`../${name}`, import.meta.url), "utf8");
const data = JSON.parse(await read("src/library-releases.json"));
test("library evidence discloses incomplete implementations and external boundaries", () => {
  const byName = Object.fromEntries(data.libraries.map(row => [row.name,row]));
  assert.match(byName.monacolil.scope, /partial.*workers excluded/i);
  assert.match(byName.solidlil.scope, /experimental.*JavaScript providers/i);
  assert.match(byName.playcanvaslil.scope, /shader.*not the complete engine/i);
  assert.match(byName.motionlil.scope, /no React integration/i);
  assert.match(byName.zodlil.scope, /locale provider external/i);
  assert.match(byName['react-markdownlil'].scope, /React external/i);
  assert.match(byName.posthoglil.scope, /seven submodules/i);
});
test("library page links all measured ports, three codecs and build times", async () => {
  const page = await read("libraries.html");
  for (const library of data.libraries) {
    assert.ok(page.includes(library.homepage), library.name);
    assert.ok(page.includes(library.evidence), library.name);
  }
  assert.match(page, /id="build-times"/);
  assert.match(page, /gzip-9/);
  assert.match(page, /Brotli-11/);
  assert.doesNotMatch(page, /previous compiler|previous release|data-popular-candidates/);
});
