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
const landingCards = JSON.parse(await read("src/landing-cards.json"));

test("the product site gives every major surface a production entry", () => {
  for (const entry of [
    "home",
    "language",
    "compare",
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
    assert.match(home, /href="\/language\.html"/);
    assert.match(home, /href="\/compare\.html"/);
    assert.match(home, /href="\/demos\.html"/);
    assert.match(home, /href="\/playground\.html"/);
    assert.match(home, /href="\/lilastro\.html"/);
    assert.match(home, /href="\/lastro\.html"/);
    assert.match(home, /href="\/solidlil\.html"/);
    assert.match(home, /href="\/demos\.html#lastro"/);
    assert.match(home, /href="\/demos\.html#solidlil-keyed"/);
    assert.match(home, /href="\/demos\.html#motion-showcase-carousel"/);
    assert.match(home, /https:\/\/yeargun\.github\.io\/solidlil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/motionlil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/mobxlil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/jquerylil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/markedlil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/zodlil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/posthoglil\//);
    assert.match(home, /https:\/\/yeargun\.github\.io\/monacolil\//);
    assert.match(home, /href="\/delivery\.html"/);
    assert.match(home, /https:\/\/github\.com\/yeargun\/lilscript/);
    assert.match(home, /Star the repo/);
    assert.match(home, /class="repo-star"/);
    assert.match(home, /class="repo-star-chip"/);
    assert.match(home, /class="repo-star-label"/);
    assert.match(home, /id="latest-title"/);
    assert.match(home, /The port pages, rebuilt by one compiler/);
    assert.match(home, /What is worse than before/);
});

test("every comparable landing project publishes recalculated gzip and Brotli rates", () => {
  const ratePattern = /<div class="(win|loss|hold) compression-rate" data-compression-rate data-baseline="(\d+)" data-candidate="(\d+)">\s*<small>([^<]+)<\/small><b>([^<]+)<\/b>/g;
  const rates = [...home.matchAll(ratePattern)].map((match) => ({
    kind: match[1],
    baseline: Number(match[2]),
    candidate: Number(match[3]),
    codec: match[4],
    displayed: match[5],
  }));
  const voting = landingCards.cards.filter((card) => card.vote !== false);

  assert.equal((home.match(/class="lib-card"/g) ?? []).length, landingCards.cards.length);
  assert.equal(rates.length, voting.length * 2);
  for (const id of ["motion", "solidlil", "mobx"]) {
    const card = landingCards.cards.find((entry) => entry.id === id);
    assert.equal(card.vote, false, id);
    const html = home.match(new RegExp(`<a\\s+[^>]*href="${card.href.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&")}"[^>]*>[\\s\\S]*?<\\/a>`))?.[0];
    assert.ok(html, id);
    assert.doesNotMatch(html, /data-compression-rate/, id);
  }

  for (const rate of rates) {
    const delta = ((rate.candidate - rate.baseline) / rate.baseline) * 100;
    const displayed = delta === 0
      ? "0.0%"
      : `${delta < 0 ? "−" : "+"}${Math.abs(delta).toFixed(1)}%`;
    assert.equal(rate.displayed, displayed, `${rate.codec}: ${rate.baseline} → ${rate.candidate}`);
    assert.equal(rate.kind, delta < 0 ? "win" : delta > 0 ? "loss" : "hold");
  }

  const medianReduction = (codec) => {
    const reductions = rates
      .filter((rate) => rate.codec.startsWith(codec))
      .map((rate) => ((rate.baseline - rate.candidate) / rate.baseline) * 100)
      .sort((a, b) => a - b);
    const middle = Math.floor(reductions.length / 2);
    return reductions.length % 2 === 1
      ? reductions[middle]
      : (reductions[middle - 1] + reductions[middle]) / 2;
  };
  const gzip = medianReduction("gzip").toFixed(1);
  const brotli = medianReduction("Brotli").toFixed(1);
  assert.match(home, new RegExp(`data-landing-median="gzip">${gzip}%<`));
  assert.match(home, new RegExp(`data-landing-median="brotli">${brotli}%<`));
  assert.match(home, new RegExp(`data-landing-count>${voting.length}<`));
  assert.match(home, new RegExp(`median result is ${gzip}% smaller with gzip and ${brotli}% smaller with Brotli`));
});

test("the landing and compare pages are rendered from landing-cards.json", () => {
  const rendered = spawnSync(process.execPath, [new URL("../scripts/landing-cards.mjs", import.meta.url).pathname, "--check"], { encoding: "utf8" });
  assert.equal(rendered.status, 0, rendered.stderr);
  for (const port of landingCards.ports) {
    assert.match(home, new RegExp(`https://yeargun\\.github\\.io/${port.name}/`), port.name);
  }
});

test("Lilastro, Lastro, and SolidLil state distinct implementation boundaries", () => {
  assert.match(lilastro, /project-local CLI/);
  assert.match(lilastro, /TypeScript is a host, not the app language/);
  assert.match(lilastro, /not a published general Astro replacement/);
  assert.match(lastro, /no separate Lastro compiler package/i);
  assert.match(lastro, /application experiment/i);
  assert.match(solidlil, /https:\/\/yeargun\.github\.io\/solidlil\//);
  assert.match(solidlil, /47-module browser runtime/);
  assert.match(solidlil, /size comparisons remain withheld/i);
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

test("language and compare pages cover syntax, config, and measured ports", () => {
  assert.match(language, /id="syntax"/);
  assert.match(language, /id="aggregates"/);
  assert.match(language, /id="mangling"/);
  assert.match(language, /javascript\.cost_model/);
  assert.match(language, /href="\/docs.html"/);
  assert.match(language, /href="\/delivery.html"/);
    assert.match(compare, /id="monaco"/);
  assert.match(compare, /887,420/);
  assert.match(compare, /413,607/);
  assert.match(compare, /Required modules<\/small><b>47/);
  assert.match(compare, /Comparison<\/small><b>withheld/);
  assert.match(compare, /25,452/);
  assert.match(compare, /9,287/);
  assert.match(compare, /id="ports"/);
  assert.match(compare, /id="previous"/);
  assert.match(compare, /id="jquery"/);
  assert.match(compare, /id="marked"/);
  assert.match(compare, /id="zod"/);
  assert.match(compare, /id="posthog"/);
  assert.match(compare, /181 <code>z<\/code> members/);
  assert.match(compare, /5,368/);
  assert.match(compare, /previous compiler, August 2026/);
  assert.match(home, /887,420/);
  assert.match(home, /Upstream modules<\/small><b>47/);
  assert.match(home, /comparison withheld/);
  assert.match(home, /25,452/);
  assert.match(home, /9,287/);
  assert.match(home, /60,281/);
  assert.match(home, /5,368/);
  assert.match(compare, /href="\/demos.html#solidlil-keyed"/);
  assert.match(compare, /href="\/demos.html#motion-showcase-carousel"/);
  assert.match(compare, /https:\/\/yeargun\.github\.io\/solidlil\//);
  assert.match(compare, /https:\/\/yeargun\.github\.io\/monacolil\//);
  assert.match(compare, /https:\/\/yeargun\.github\.io\/markedlil\//);
  assert.match(compare, /https:\/\/yeargun\.github\.io\/zodlil\//);
  assert.match(compare, /https:\/\/yeargun\.github\.io\/posthoglil\//);
  assert.match(compare, /href="\/delivery.html"/);
  assert.match(compare, /mangle: true<\/code> means identifier mangling only/);
  assert.match(compare, /Terser property mangling is a\s+separate option and is off here/);
  assert.match(compare, /Today’s compiler renames no properties at all/);
  assert.doesNotMatch(compare, /Oxc closer-world/);
  assert.doesNotMatch(compare, /Oxc mangle<\/small>/);
  assert.doesNotMatch(home, /5–10%/);
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
