import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const [
  config,
  home,
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
  read("lilastro.html"),
  read("lastro.html"),
  read("solidlil.html"),
  read("marketplace.html"),
  read("src/marketplace.js"),
  read("src/solid-api-parity.json").then(JSON.parse),
  read("src/solid-lsx-parity.json").then(JSON.parse),
]);

test("the product site gives every major surface a production entry", () => {
  for (const entry of [
    "home",
    "demos",
    "playground",
    "lilastro",
    "lastro",
    "solidlil",
    "marketplace",
  ]) {
    assert.match(config, new RegExp(`${entry}: resolve`), entry);
  }
  assert.match(home, /LilScript is a typed, compression-first language/);
  assert.match(home, /href="\/demos\.html"/);
  assert.match(home, /href="\/playground\.html"/);
  assert.match(home, /href="\/lilastro\.html"/);
  assert.match(home, /href="\/lastro\.html"/);
  assert.match(home, /href="\/solidlil\.html"/);
  assert.match(home, /href="\/demos\.html#lastro"/);
  assert.match(home, /href="\/demos\.html#solidlil-keyed"/);
});

test("Lilastro, Lastro, and SolidLil state distinct implementation boundaries", () => {
  assert.match(lilastro, /project-local CLI/);
  assert.match(lilastro, /TypeScript is a host, not the app language/);
  assert.match(lilastro, /not a published general Astro replacement/);
  assert.match(lastro, /no separate Lastro compiler package/i);
  assert.match(lastro, /application experiment/i);
  assert.match(solidlil, /135\/135\s+public\s+exports/);
  assert.match(solidlil, /469\/469\s+unchanged\s+upstream\s+tests/);
  assert.match(solidlil, /Runtime \+ client LSX parity/);
  assert.match(solidlil, /data-solid-api-parity/);
  assert.match(solidlil, /data-solid-runtime-results/);
  assert.match(solidlil, /46-export client Web target/);
  assert.match(solidlil, /full\s+73-export compatibility bundle/);
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
