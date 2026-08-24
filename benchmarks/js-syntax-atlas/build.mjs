import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
} from "../codec-contract.mjs";
import {
  COMPRESSION_CONTEXT,
  REPEAT_COUNT,
  races,
} from "./catalog.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const templatePath = join(directory, "template.html");
const reportPath = join(directory, "report.html");
const checkOnly = process.argv.includes("--check");
const allowedSafety = new Set(["contract", "narrow", "trap"]);
const allowedMetrics = ["raw", "gzip", "brotli"];
const allowedLanes = ["single", "repeated", "context"];

function validateCatalog() {
  assert.ok(races.length >= 50, "the atlas should stay broad");
  const raceIds = new Set();
  for (const item of races) {
    assert.match(item.id, /^[a-z0-9-]+$/u, `${item.id} id`);
    assert.ok(!raceIds.has(item.id), `duplicate race id: ${item.id}`);
    raceIds.add(item.id);
    assert.ok(item.group.length > 0, `${item.id} group`);
    assert.ok(item.title.length > 0, `${item.id} title`);
    assert.ok(item.contract.length > 0, `${item.id} contract`);
    assert.ok(item.caveat.length > 0, `${item.id} caveat`);
    assert.ok(item.variants.length >= 2, `${item.id} variants`);
    assert.ok(
      item.variants.some((variant) => variant.safety !== "trap"),
      `${item.id} needs a rankable variant`,
    );
    const names = new Set();
    for (const variant of item.variants) {
      assert.ok(!names.has(variant.name), `${item.id} duplicate ${variant.name}`);
      names.add(variant.name);
      assert.ok(allowedSafety.has(variant.safety), `${item.id}/${variant.name} safety`);
      assert.equal(variant.code.trim(), variant.code, `${item.id}/${variant.name} whitespace`);
      assert.ok(!variant.code.endsWith(";"), `${item.id}/${variant.name} trailing semicolon`);
      assert.doesNotThrow(
        () => new Function(variant.code),
        `${item.id}/${variant.name} must parse as a function body`,
      );
    }
  }
  assert.doesNotThrow(
    () => new Function(COMPRESSION_CONTEXT),
    "compression context must parse",
  );
}

function repeatedSource(code) {
  return Array.from({ length: REPEAT_COUNT }, () => `{${code}}`).join("\n");
}

function contextualSource(code) {
  return `${COMPRESSION_CONTEXT}\n${code}`;
}

function minimumVariantIds(item, lane, metric) {
  const eligible = item.variants.filter((variant) => variant.safety !== "trap");
  const minimum = Math.min(...eligible.map((variant) => variant.sizes[lane][metric]));
  return eligible
    .filter((variant) => variant.sizes[lane][metric] === minimum)
    .map((variant) => variant.id);
}

function sameSet(left, right) {
  return left.length === right.length && left.every((value) => right.includes(value));
}

function makeSummary(measuredRaces) {
  const disagreements = Object.fromEntries(
    allowedLanes.map((lane) => [
      lane,
      {
        rawVsGzip: 0,
        rawVsBrotli: 0,
        gzipVsBrotli: 0,
        longerRawWinsGzip: 0,
        longerRawWinsBrotli: 0,
      },
    ]),
  );
  for (const item of measuredRaces) {
    for (const lane of allowedLanes) {
      const winners = Object.fromEntries(
        allowedMetrics.map((metric) => [metric, minimumVariantIds(item, lane, metric)]),
      );
      if (!sameSet(winners.raw, winners.gzip)) disagreements[lane].rawVsGzip += 1;
      if (!sameSet(winners.raw, winners.brotli)) disagreements[lane].rawVsBrotli += 1;
      if (!sameSet(winners.gzip, winners.brotli)) disagreements[lane].gzipVsBrotli += 1;
      const rawMinimum = Math.min(
        ...item.variants
          .filter((variant) => variant.safety !== "trap")
          .map((variant) => variant.sizes[lane].raw),
      );
      for (const metric of ["gzip", "brotli"]) {
        const compressedWinners = item.variants.filter((variant) =>
          winners[metric].includes(variant.id),
        );
        if (compressedWinners.every((variant) => variant.sizes[lane].raw > rawMinimum)) {
          disagreements[lane][metric === "gzip" ? "longerRawWinsGzip" : "longerRawWinsBrotli"] += 1;
        }
      }
    }
  }
  const laneFlips = {};
  for (const metric of allowedMetrics) {
    laneFlips[metric] = { singleVsRepeated: 0, singleVsContext: 0 };
    for (const item of measuredRaces) {
      const single = minimumVariantIds(item, "single", metric);
      if (!sameSet(single, minimumVariantIds(item, "repeated", metric))) {
        laneFlips[metric].singleVsRepeated += 1;
      }
      if (!sameSet(single, minimumVariantIds(item, "context", metric))) {
        laneFlips[metric].singleVsContext += 1;
      }
    }
  }
  return { disagreements, laneFlips };
}

validateCatalog();
const temporaryDirectory = mkdtempSync(join(tmpdir(), "lilscript-js-atlas-"));
try {
  const artifacts = [];
  const descriptors = [];
  const contextPath = join(temporaryDirectory, "context.js");
  writeFileSync(contextPath, COMPRESSION_CONTEXT);
  artifacts.push(contextPath);
  descriptors.push({ kind: "base" });

  races.forEach((item, raceIndex) => {
    item.variants.forEach((variant, variantIndex) => {
      const laneSources = {
        single: variant.code,
        repeated: repeatedSource(variant.code),
        context: contextualSource(variant.code),
      };
      for (const lane of allowedLanes) {
        const path = join(
          temporaryDirectory,
          `${String(raceIndex).padStart(2, "0")}-${String(variantIndex).padStart(2, "0")}-${lane}.js`,
        );
        writeFileSync(path, laneSources[lane]);
        artifacts.push(path);
        descriptors.push({ kind: "variant", raceIndex, variantIndex, lane });
      }
    });
  });

  const measurements = canonicalCodecMeasurementsForFiles(
    artifacts,
    "JavaScript syntax atlas",
  );
  const baseMeasurement = measurements[0];
  const measuredRaces = races.map((item, raceIndex) => ({
    ...item,
    variants: item.variants.map((variant, variantIndex) => ({
      ...variant,
      id: `${item.id}-${variantIndex + 1}`,
      sizes: {},
    })),
  }));

  for (let index = 1; index < measurements.length; index += 1) {
    const descriptor = descriptors[index];
    const measurement = measurements[index];
    const target =
      measuredRaces[descriptor.raceIndex].variants[descriptor.variantIndex];
    const sizes = Object.fromEntries(
      allowedMetrics.map((metric) => {
        const value =
          descriptor.lane === "context"
            ? measurement[metric] - baseMeasurement[metric]
            : measurement[metric];
        return [metric, value];
      }),
    );
    target.sizes[descriptor.lane] = sizes;
  }

  for (const item of measuredRaces) {
    for (const variant of item.variants) {
      assert.deepEqual(Object.keys(variant.sizes).sort(), [...allowedLanes].sort());
      assert.equal(variant.sizes.single.raw, Buffer.byteLength(variant.code));
      assert.equal(
        variant.sizes.context.raw,
        Buffer.byteLength(variant.code) + 1,
        `${item.id}/${variant.name} contextual raw marginal`,
      );
    }
  }

  const payload = {
    schemaVersion: 1,
    title: "JavaScript compression syntax atlas",
    scope: "Pre-minified interchangeable spellings; semantic contracts are part of every comparison.",
    lanes: {
      single: {
        label: "Single spelling",
        description: "The exact displayed snippet as its own compressed stream.",
      },
      repeated: {
        label: `${REPEAT_COUNT}× repeated`,
        description: `${REPEAT_COUNT} block-scoped copies in one compressed stream.`,
      },
      context: {
        label: "In app context",
        description: "Marginal bytes after appending the snippet to the fixed background corpus.",
      },
    },
    repeatCount: REPEAT_COUNT,
    context: {
      bytes: Buffer.byteLength(COMPRESSION_CONTEXT),
      sha256: createHash("sha256").update(COMPRESSION_CONTEXT).digest("hex"),
      source: COMPRESSION_CONTEXT,
      sizes: Object.fromEntries(
        allowedMetrics.map((metric) => [metric, baseMeasurement[metric]]),
      ),
    },
    codecs: canonicalCodecProvenance("JavaScript syntax atlas report"),
    counts: {
      races: measuredRaces.length,
      variants: measuredRaces.reduce((total, item) => total + item.variants.length, 0),
      groups: new Set(measuredRaces.map((item) => item.group)).size,
    },
    summary: makeSummary(measuredRaces),
    races: measuredRaces,
  };

  const template = readFileSync(templatePath, "utf8");
  assert.equal(
    template.match(/__ATLAS_DATA__/gu)?.length,
    1,
    "template must contain exactly one data placeholder",
  );
  const serialized = JSON.stringify(payload).replaceAll("<", "\\u003c");
  const report = template.replace("__ATLAS_DATA__", serialized);
  if (checkOnly) {
    assert.equal(readFileSync(reportPath, "utf8"), report, "report.html is stale");
  } else {
    writeFileSync(reportPath, report);
  }
  process.stdout.write(
    `${checkOnly ? "Verified" : "Built"} ${payload.counts.races} races / ` +
      `${payload.counts.variants} variants in ${reportPath}\n`,
  );
} finally {
  rmSync(temporaryDirectory, { recursive: true, force: true });
}
