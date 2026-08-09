import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const delivery = await readFile(new URL("../delivery.html", import.meta.url), "utf8");
const script = await readFile(new URL("../src/delivery.js", import.meta.url), "utf8");
const config = await readFile(new URL("../vite.config.js", import.meta.url), "utf8");

test("delivery architecture is a production entry with accessible strategy panels", () => {
  assert.match(config, /rolldownOptions/);
  assert.match(config, /delivery: resolve/);
  assert.match(delivery, /data-strategy-panel="static"/);
  assert.match(delivery, /data-strategy-panel="phase"/);
  assert.match(delivery, /data-strategy-panel="extract"/);
  assert.match(delivery, /data-strategy-panel="capsule"/);
  assert.match(delivery, /role="tablist"/);
  assert.match(script, /aria-selected/);
  assert.match(script, /ArrowLeft/);
  assert.match(script, /ArrowRight/);
});

test("delivery page states dependency, injection, cache, and multi-target boundaries", () => {
  assert.match(delivery, /must fetch and evaluate the required B module graph before evaluating A/);
  assert.match(delivery, /Cannot satisfy “evaluate A, then load B.”/);
  assert.match(delivery, /“Inject it into A” has four different meanings/);
  assert.match(delivery, /Mutable state and live exports have one authoritative owner/);
  assert.match(delivery, /CacheStorage is separate from HTTP cache/);
  assert.match(delivery, /Shared logic; capability-specific delivery/);
  assert.match(delivery, /proposed syntax, not a shipped promise/i);
  assert.match(delivery, /Folders are useful affinity/);
  assert.match(delivery, /Manual constraints are essential/);
  assert.match(delivery, /ActivationGraph → OwnershipPlan → TransferPlan → Emission/);
  assert.match(delivery, /Download early\. Register safely\. Execute by readiness\./);
  assert.match(delivery, /Capsule A has no native static edge to B/);
  assert.match(delivery, /No <code>eval<\/code>/);
  assert.match(delivery, /benchmark must include scheduler and registration-wrapper bytes/);
});

test("every primary page links to the delivery architecture", async () => {
  const pages = [
    "index.html", "docs.html", "benchmarks.html", "libraries.html", "explorer.html",
    "benchmark-detail.html", "delivery.html", "roadmap.html", "about.html",
  ];
  for (const page of pages) {
    const html = await readFile(new URL(`../${page}`, import.meta.url), "utf8");
    assert.match(html, /href="\/delivery\.html"/, page);
  }
});
