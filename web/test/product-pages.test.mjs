import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { spawnSync } from "node:child_process";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const [
  config,
  home,
  language,
  compare,
  lilastro,
  lastro,
  solidlil,
  marketplace,
  marketplaceScript,
  solidParity,
  lsxParity,
] = await Promise.all([
  read("vite.config.js"),
  read("index.html"),
  read("language.html"),
  read("compare.html"),
  read("lilastro.html"),
  read("lastro.html"),
  read("solidlil.html"),
  read("marketplace.html"),
  read("src/marketplace.js"),
  read("src/solid-api-parity.json").then(JSON.parse),
  read("src/solid-lsx-parity.json").then(JSON.parse),
]);
const releases = JSON.parse(await read("src/library-releases.json"));

test("current library evidence covers every listed port and codec", async () => {
  assert.equal(releases.libraries.length, 26);
  assert.equal(new Set(releases.libraries.map(row => row.name)).size, 26);
  assert.deepEqual(JSON.parse(await read("public/library-releases.json")), releases);
  for (const row of releases.libraries) {
    assert.deepEqual(row.objectives.map(item => item.objective), ["raw", "gzip", "brotli"]);
    assert.ok(row.scope.length > 20);
    assert.ok(row.packageEvidence && row.packageVersion);
    for (const item of row.objectives) {
      assert.ok(item.originalBytes > 0 && item.lilscriptBytes > 0);
      assert.ok(item.originalBuildSeconds > 0 && item.lilscriptBuildSeconds > 0);
      assert.match(item.lilscriptSha256, /^[a-f0-9]{64}$/);
      assert.match(item.originalSha256, /^[a-f0-9]{64}$/);
    }
    assert.ok(home.includes(row.homepage), row.name);
    assert.ok(compare.includes(row.homepage), row.name);
  }
});

test("landing cards show all raw, gzip and Brotli rates, including losses", () => {
  const rates = [...home.matchAll(/<div class="(win|loss|hold) compression-rate" data-compression-rate data-baseline="(\d+)" data-candidate="(\d+)"><small>([^<]+)<\/small><b>([^<]+)<\/b>/g)];
  assert.equal(rates.length, 26 * 3);
  for (const [,kind,baseline,candidate,codec,displayed] of rates) {
    const delta = (Number(candidate) / Number(baseline) - 1) * 100;
    assert.equal(displayed, `${delta < 0 ? "−" : delta > 0 ? "+" : ""}${Math.abs(delta).toFixed(1)}%`, codec);
    assert.equal(kind, delta < 0 ? "win" : delta > 0 ? "loss" : "hold");
  }
});

test("public comparison pages are generated from the published evidence index", () => {
  const result = spawnSync(process.execPath, [new URL("../scripts/landing-cards.mjs", import.meta.url).pathname, "--check"], {encoding: "utf8"});
  assert.equal(result.status, 0, result.stderr);
  assert.match(compare, /id="build-times"/);
  assert.match(compare, /bundle.*minif/i);
  assert.match(compare, /separate.*compil/i);
  for (const page of [home, compare, language]) {
    assert.doesNotMatch(page, /previous compiler|previous version|worse than before|renames no properties|javascript\.cost_model/i);
  }
});

test("Lilastro, Lastro, and SolidLil state distinct implementation boundaries", () => {
  assert.match(lilastro, /project-local CLI/);
  assert.match(lilastro, /TypeScript is a host, not the app language/);
  assert.match(lilastro, /not a published general Astro replacement/);
  assert.match(lastro, /no separate Lastro compiler package/i);
  assert.match(lastro, /application experiment/i);
  assert.match(solidlil, /https:\/\/yeargun\.github\.io\/solidlil\//);
  assert.match(solidlil, /Compatibility coverage is not exact parity/);
  assert.doesNotMatch(solidlil, /data-solid-api-parity/);
  assert.doesNotMatch(solidlil, /data-solid-runtime-results/);
  assert.equal(solidParity.complete, true);
  assert.equal(solidParity.totals.expected, 135);
  assert.equal(solidParity.totals.verified, 135);
  assert.equal(lsxParity.complete, true);
  assert.equal(lsxParity.counts.inventory, 23);
  assert.equal(lsxParity.counts.expected, 21);
  assert.equal(lsxParity.counts.excluded, 2);
  assert.equal(lsxParity.counts.loweringVerified, 21);
  assert.equal(lsxParity.counts.runtimeVerified, 21);
});

test("language documentation uses the current compiler policy and objective", () => {
  for (const id of ["syntax", "aggregates", "mangling"]) assert.ok(language.includes(`id="${id}"`));
  assert.match(language, /objective/);
  assert.match(language, /codecs/);
  assert.match(language, /version = 3/);
  assert.match(language, /jobs = 1/);
});

test("Parcel Market starts accessibly and keeps the fake-payment boundary explicit", () => {
  assert.match(marketplace, /class="skip-link"/);
  assert.match(
    marketplace,
    /role="status" aria-live="polite" aria-atomic="true"/,
  );
  assert.match(marketplace, /aria-busy="true"/);
  assert.match(marketplace, /No login, backend, persistence, or real payment/);
  assert.match(
    marketplaceScript,
    /aria-label="Remove one \$\{listing\.name\} from shopping list"/,
  );
  assert.match(marketplaceScript, /data-payment-form/);
  assert.match(
    marketplaceScript,
    /No card was charged and no order was placed/,
  );
  assert.match(marketplaceScript, /requestAnimationFrame/);
  assert.doesNotMatch(marketplaceScript, /fetch\s*\(/);
});
