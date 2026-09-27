// Render the landing page's library cards from src/landing-cards.json and
// recompute every median the page quotes. Run after changing the JSON:
//
//   node scripts/landing-cards.mjs          # rewrite index.html and compare.html
//   node scripts/landing-cards.mjs --check  # exit 1 if either is stale
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const data = JSON.parse(readFileSync(join(webRoot, "src/landing-cards.json"), "utf8"));
const number = new Intl.NumberFormat("en-US");

export const rate = (baseline, candidate) => {
  const delta = ((candidate - baseline) / baseline) * 100;
  if (delta === 0) return "0.0%";
  return `${delta < 0 ? "−" : "+"}${Math.abs(delta).toFixed(1)}%`;
};

export function medianReduction(cards, codec) {
  const values = cards
    .filter((card) => card.vote !== false)
    .map((card) => ((card[codec][0] - card[codec][1]) / card[codec][0]) * 100)
    .sort((a, b) => a - b);
  const middle = Math.floor(values.length / 2);
  return values.length % 2 ? values[middle] : (values[middle - 1] + values[middle]) / 2;
}

const indent = (depth) => " ".repeat(depth);

function rateBlock(card, codec, label) {
  const [baseline, candidate] = card[codec];
  const kind = candidate < baseline ? "win" : candidate > baseline ? "loss" : "hold";
  return [
    `${indent(14)}<div class="${kind} compression-rate" data-compression-rate data-baseline="${baseline}" data-candidate="${candidate}">`,
    `${indent(16)}<small>${label}</small><b>${rate(baseline, candidate)}</b><span>${number.format(baseline)} → ${number.format(candidate)} B</span>`,
    `${indent(14)}</div>`,
  ].join("\n");
}

function renderCard(card) {
  const open = card.external
    ? `${indent(10)}<a\n${indent(12)}class="lib-card"\n${indent(12)}href="${card.href}"\n${indent(12)}target="_blank"\n${indent(12)}rel="noopener"\n${indent(10)}>`
    : `${indent(10)}<a class="lib-card" href="${card.href}">`;
  const metrics =
    card.vote === false
      ? [
          `${indent(12)}<div class="lib-card-metrics">`,
          ...card.metrics.map(
            ([label, value, note]) =>
              `${indent(14)}<div><small>${label}</small><b>${value}</b>${note ? `<span>${note}</span>` : ""}</div>`,
          ),
          `${indent(12)}</div>`,
        ]
      : [
          `${indent(12)}<div class="lib-card-metrics compression-rates">`,
          rateBlock(card, "gzip", `gzip-9 · vs ${card.baselineLabel}`),
          rateBlock(card, "brotli", `Brotli-11 · vs ${card.baselineLabel}`),
          `${indent(12)}</div>`,
        ];
  return [
    open,
    `${indent(12)}<span class="lib-card-kicker">${card.kicker}</span>`,
    `${indent(12)}<h3>${card.title}</h3>`,
    `${indent(12)}<p>${card.text}</p>`,
    ...metrics,
    `${indent(10)}</a>`,
  ].join("\n");
}

// One row of the all-ports table: the headline file each page publishes, its
// bar, and which compiler built the published file.
function renderPortRow(port) {
  const bytes = (value) => (value == null ? "—" : `${number.format(value)} B`);
  const delta =
    port.now == null || port.bar == null
      ? `<td>—</td>`
      : `<td class="${port.now < port.bar ? "win" : port.now > port.bar ? "loss" : "hold"}">${rate(port.bar, port.now)}</td>`;
  const link = `<a href="https://yeargun.github.io/${port.name}/" target="_blank" rel="noopener">${port.name}</a>`;
  return [
    `${indent(14)}<tr>`,
    `${indent(16)}<th scope="row">${link}<small>${port.package}</small></th>`,
    `${indent(16)}<td><code>${port.file}</code></td>`,
    `${indent(16)}<td>${bytes(port.now)}</td>`,
    `${indent(16)}<td>${bytes(port.bar)}<small>${port.barLabel}</small></td>`,
    `${indent(16)}${delta}`,
    `${indent(16)}<td>${port.builtBy}${port.note ? `<small>${port.note}</small>` : ""}</td>`,
    `${indent(14)}</tr>`,
  ].join("\n");
}

export function render(html, { describe = false } = {}) {
  const voting = data.cards.filter((card) => card.vote !== false);
  const gzip = medianReduction(data.cards, "gzip").toFixed(1);
  const brotli = medianReduction(data.cards, "brotli").toFixed(1);
  const cards = data.cards.map(renderCard).join("\n");
  let next = html.replace(
    /(<!-- landing-cards:start -->)[\s\S]*?(\n[ ]*<!-- landing-cards:end -->)/,
    `$1\n${cards}$2`,
  );
  next = next.replace(
    /(<!-- landing-ports:start -->)[\s\S]*?(\n[ ]*<!-- landing-ports:end -->)/,
    `$1\n${data.ports.map(renderPortRow).join("\n")}$2`,
  );
  next = next.replace(/data-landing-median="gzip">[^<]*</g, `data-landing-median="gzip">${gzip}%<`);
  next = next.replace(/data-landing-median="brotli">[^<]*</g, `data-landing-median="brotli">${brotli}%<`);
  next = next.replace(/data-landing-count>[^<]*</g, `data-landing-count>${voting.length}<`);
  if (describe) next = next.replace(
    /(<meta\s+name="description"\s+content=")[^"]*(")/,
    `$1LilScript is a typed language that compiles to smaller JavaScript. Across ${voting.length} library rows measured with today’s compiler, the median result is ${gzip}% smaller with gzip and ${brotli}% smaller with Brotli than the bar named on each card.$2`,
  );
  return next;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let stale = false;
  for (const name of ["index.html", "compare.html"]) {
    const path = join(webRoot, name);
    const html = readFileSync(path, "utf8");
    if (!html.includes("<!-- landing-cards:start -->")) throw new Error(`${name} has no landing-cards markers`);
    const next = render(html, { describe: name === "index.html" });
    if (next === html) continue;
    stale = true;
    if (!process.argv.includes("--check")) writeFileSync(path, next);
  }
  if (process.argv.includes("--check")) {
    if (stale) {
      console.error("index.html or compare.html is stale: run node scripts/landing-cards.mjs");
      process.exit(1);
    }
  } else {
    console.log(
      `rendered ${data.cards.length} cards; medians gzip ${medianReduction(data.cards, "gzip").toFixed(1)}% brotli ${medianReduction(data.cards, "brotli").toFixed(1)}%`,
    );
  }
}
