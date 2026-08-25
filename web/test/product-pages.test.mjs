import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

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
    assert.match(home, /Global compression search/);
    assert.match(home, /posthog-js adds two browser packs/);
});

test("every comparable landing project publishes recalculated gzip and Brotli rates", () => {
  const ratePattern = /<div class="(?:win|loss|hold) compression-rate" data-compression-rate data-baseline="(\d+)" data-candidate="(\d+)">\s*<small>([^<]+)<\/small><b>([^<]+)<\/b>/g;
  const rates = [...home.matchAll(ratePattern)].map((match) => ({
    baseline: Number(match[1]),
    candidate: Number(match[2]),
    codec: match[3],
    displayed: match[4],
  }));

  assert.equal((home.match(/class="lib-card"/g) ?? []).length, 16);
  assert.equal(rates.length, 30);
  const zodCard = home.match(/<a\s+[^>]*href="https:\/\/yeargun\.github\.io\/zodlil\/"[^>]*>[\s\S]*?<\/a>/)?.[0];
  assert.ok(zodCard);
  assert.doesNotMatch(zodCard, /data-compression-rate/);
  assert.match(zodCard, /Size claim<\/small><b>Withdrawn/);

  for (const rate of rates) {
    const delta = ((rate.candidate - rate.baseline) / rate.baseline) * 100;
    const displayed = delta === 0
      ? "0.0%"
      : `${delta < 0 ? "−" : "+"}${Math.abs(delta).toFixed(1)}%`;
    assert.equal(rate.displayed, displayed, `${rate.codec}: ${rate.baseline} → ${rate.candidate}`);
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

  assert.equal(medianReduction("gzip").toFixed(1), "7.7");
  assert.equal(medianReduction("Brotli").toFixed(1), "7.5");
  assert.match(home, /median project result is 7\.7% smaller under\s+gzip-9 and 7\.5% smaller under Brotli-11/);
  assert.match(home, /Zod stays visible but has no vote/);
});

test("Lilastro, Lastro, and SolidLil state distinct implementation boundaries", () => {
  assert.match(lilastro, /project-local CLI/);
  assert.match(lilastro, /TypeScript is a host, not the app language/);
  assert.match(lilastro, /not a published general Astro replacement/);
  assert.match(lastro, /no separate Lastro compiler package/i);
  assert.match(lastro, /application experiment/i);
  assert.match(solidlil, /https:\/\/yeargun\.github\.io\/solidlil\//);
  assert.match(solidlil, /11,180/);
  assert.match(solidlil, /3,862/);
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
  assert.match(compare, /11,180/);
  assert.match(compare, /3,862/);
  assert.match(compare, /30,741/);
  assert.match(compare, /9,515/);
  assert.match(compare, /id="jquery"/);
  assert.match(compare, /id="marked"/);
  assert.match(compare, /id="zod"/);
  assert.match(compare, /id="posthog"/);
  assert.match(compare, /62,763/);
  assert.match(compare, /52,583/);
  assert.match(compare, /192 ESM names versus 240/);
  assert.match(compare, /Size claim[\s\S]*Withdrawn/);
  assert.match(compare, /5,622/);
  assert.match(compare, /5,985/);
  assert.match(compare, /4,215/);
  assert.match(compare, /3,186/);
  assert.match(compare, /4,258/);
  assert.match(compare, /3,465/);
  assert.match(home, /887,420/);
  assert.match(home, /11,180/);
  assert.match(home, /30,741/);
  assert.match(home, /9,515/);
  assert.match(home, /62,763/);
  assert.match(home, /52,583/);
  assert.match(home, /Official exports<\/small><b>240/);
  assert.match(home, /Port exports<\/small><b>192/);
  assert.match(home, /5,622/);
  assert.match(home, /5,985/);
  assert.match(home, /24\.4%/);
  assert.match(home, /18\.6%/);
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
