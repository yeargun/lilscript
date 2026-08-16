import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  allDemoPairs,
  demoById,
  demoGroups,
  demos,
} from "../src/demos-catalog.js";

const [page, script, config, site] = await Promise.all([
  readFile(new URL("../demos.html", import.meta.url), "utf8"),
  readFile(new URL("../src/demos.js", import.meta.url), "utf8"),
  readFile(new URL("../vite.config.js", import.meta.url), "utf8"),
  readFile(new URL("../src/site.js", import.meta.url), "utf8"),
]);

test("demos is a Vite entry with a dual-frame stage", () => {
  assert.match(config, /demos: resolve/);
  assert.match(page, /data-demo-rail/);
  assert.match(page, /data-demo-stage/);
  assert.match(page, /data-demo-filters/);
  assert.match(script, /demo-stage-frames/);
  assert.match(script, /demo-source/);
  assert.match(site, /\["Demos", "\/demos\.html"\]/);
});

test("featured demos lead with Lastro and SolidLil pairs", () => {
  assert.equal(demos[0].id, "lastro");
  assert.equal(demos[1].id, "solidlil-keyed");
  assert.equal(demos[2].id, "solidlil-lsx");
  assert.equal(demos[3].id, "solidlil-api");
  assert.equal(demoById("lastro").candidate.url, "/marketplace.html?embed=1");
  assert.equal(demoById("lastro").baseline.url, "/demos/lastro-astro/index.html");
  assert.equal(demoById("solidlil-keyed").baseline.url, "/demos/keyed-solid/index.html");
  assert.equal(demoById("solidlil-keyed").candidate.url, "/demos/keyed-solidlil/index.html");
  assert.ok(demoById("solidlil-lsx").candidate.sizes.brotli < demoById("solidlil-lsx").baseline.sizes.brotli);
  assert.ok(demoById("solidlil-api").candidate.facts.length >= 3);
});

test("the gallery groups motion and keeps live pairs addressable", () => {
  const ids = new Set(demos.map((demo) => demo.id));
  const pairIds = new Set(allDemoPairs().map((demo) => demo.id));
  assert.equal(ids.size, demos.length);
  assert.ok(ids.has("motion-showcases"));
  assert.ok(ids.has("motion-animate"));
  assert.ok(ids.has("motion-interaction"));
  assert.ok(pairIds.has("motion-showcase-carousel"));
  assert.ok(ids.has("lib-nanoid"));
  assert.ok(ids.has("lib-jquery"));
  assert.ok(ids.has("port-micro-math"));
  assert.ok(ids.has("port-motion-easing"));
  assert.ok(pairIds.has("algo-aggregate-ledger"));
  assert.ok(pairIds.has("paired-boolean-literals"));
  assert.ok(ids.has("app-login-risk"));
  assert.equal(demoGroups.length, 5);
  assert.equal(demos.find((demo) => demo.id === "motion-showcases").variants.length, 5);
  assert.equal(
    demos
      .filter((demo) => demo.group === "motion")
      .flatMap((demo) => demo.variants).length,
    16,
  );
  assert.ok(allDemoPairs().filter((demo) => demo.kind === "visual").length >= 16);
  assert.ok(
    allDemoPairs()
      .filter((demo) => demo.baseline.url && demo.candidate.url)
      .every((demo) => demo.baseline.url.includes(".html") && demo.candidate.url.includes(".html")),
  );
  assert.ok(demos.filter((demo) => demo.group === "libraries").length >= 12);
  assert.ok(demos.every((demo) => demo.baseline && demo.candidate && demo.source?.href.includes("github.com/yeargun/lilscript")));
});
