import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  artifactForMetric,
  assertMatrix,
  assertResult,
  buildComparisons,
  canonicalResult,
  evidenceFingerprint,
  sha256,
  stableJson,
} from "./contract.mjs";
import { seedSource } from "./results/seed-source.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const matrixBytes = readFileSync(join(here, "matrix.json"));
const matrix = JSON.parse(matrixBytes);
const schema = JSON.parse(readFileSync(join(here, "schema.json"), "utf8"));
const seed = JSON.parse(readFileSync(join(here, "results/seed.json"), "utf8"));

test("the pinned matrix and immutable seed satisfy the executable contract", () => {
  assert.doesNotThrow(() => assertMatrix(matrix));
  assert.equal(seed.matrixSha256, sha256(matrixBytes));
  assert.doesNotThrow(() => assertResult(seed, matrix));
});

test("every local JSON Schema reference resolves", () => {
  const references = [];
  const visit = (value) => {
    if (Array.isArray(value)) {
      value.forEach(visit);
    } else if (value && typeof value === "object") {
      if (typeof value.$ref === "string") references.push(value.$ref);
      Object.values(value).forEach(visit);
    }
  };
  visit(schema);
  for (const reference of references) {
    assert.match(reference, /^#\//u);
    let current = schema;
    for (const token of reference.slice(2).split("/")) {
      current = current[token.replaceAll("~1", "/").replaceAll("~0", "~")];
      assert.notEqual(current, undefined, `unresolved schema reference ${reference}`);
    }
  }
  assert.ok(schema.required.includes("regressionPolicy"));
  assert.ok(schema.$defs.artifact.required.includes("semantic"));
});

test("the reviewed seed source deterministically reproduces seed.json", () => {
  const regenerated = canonicalResult(seedSource(sha256(matrixBytes)), matrix);
  assert.equal(stableJson(regenerated), stableJson(seed));
});

test("every configured objective has exactly one honest artifact lane", () => {
  const marked = matrix.libraries.find((library) => library.id === "markedlil");
  assert.deepEqual(
    marked.build.artifacts.map(({ objective, gateMetrics, configPath }) => ({
      objective,
      gateMetrics,
      configPath,
    })),
    [
      {
        objective: "raw",
        gateMetrics: ["raw"],
        configPath: "lilscript.bytes.toml",
      },
      {
        objective: "gzip9",
        gateMetrics: ["gzip9"],
        configPath: "lilscript.gzip.toml",
      },
      {
        objective: "brotli11",
        gateMetrics: ["brotli11"],
        configPath: "lilscript.toml",
      },
    ],
  );
  assert.equal(marked.semantic.scope, "artifact");
  for (const library of matrix.libraries.filter(
    (item) => item.id !== "markedlil",
  )) {
    assert.equal(library.build.artifacts.length, 1);
    assert.equal(library.build.artifacts[0].objective, "brotli11");
    assert.deepEqual(library.build.artifacts[0].gateMetrics, ["brotli11"]);
  }
});

test("canonical evidence is stable under input and key ordering", () => {
  const reordered = {
    observations: [...seed.observations].reverse(),
    codec: { ...seed.codec },
    regressionPolicy: structuredClone(seed.regressionPolicy),
    format: seed.format,
    schemaVersion: seed.schemaVersion,
    matrixSha256: seed.matrixSha256,
    comparisons: [...seed.comparisons].reverse(),
    evidenceFingerprint: "0".repeat(64),
  };
  const canonical = canonicalResult(reordered, matrix);
  assert.equal(canonical.evidenceFingerprint, seed.evidenceFingerprint);
  assert.equal(evidenceFingerprint(canonical), seed.evidenceFingerprint);
  assert.equal(stableJson(canonical), stableJson(seed));
});

function exactCompiler(role) {
  const specification = matrix.compilers.find((item) => item.id === role);
  return {
    role,
    revision: specification.revision,
    tree: specification.tree,
    binarySha256: role === "baseline" ? "a".repeat(64) : "b".repeat(64),
    primarySourceSha256: specification.primarySourceSha256,
    sourceIdentity: "test fixture",
  };
}

test("a metric cannot win unless its own artifact passed fresh semantics", () => {
  const published = seed.observations.find(
    (item) => item.id === "published.markedlil",
  );
  const before = structuredClone(published);
  before.id = "fixture.markedlil.baseline";
  before.purpose = "comparison";
  before.evidenceClass = "fresh";
  before.compiler = exactCompiler("baseline");
  before.semantic = {
    status: "passed",
    evidenceClass: "fresh",
    command: "fixture",
    summary: "all lanes passed",
  };
  for (const artifact of before.artifacts) {
    artifact.semantic = {
      status: "passed",
      evidenceClass: "fresh",
      command: "fixture",
      summary: "lane passed",
    };
  }

  const after = structuredClone(before);
  after.id = "fixture.markedlil.checkpoint";
  after.compiler = exactCompiler("checkpoint");
  for (const artifact of after.artifacts) {
    artifact.sizes[artifact.objective] -= 1;
  }
  artifactForMetric(after, "raw").semantic = {
    status: "not-run",
    evidenceClass: "none",
    command: null,
    summary: "this lane was not tested",
  };

  const rows = buildComparisons([before, after], {
    ...matrix,
    libraries: [matrix.libraries.find((item) => item.id === "markedlil")],
  });
  assert.equal(rows.find((row) => row.metric === "raw").outcome, "ineligible");
  assert.equal(rows.find((row) => row.metric === "gzip9").outcome, "win");
  assert.equal(rows.find((row) => row.metric === "brotli11").outcome, "win");
});

test("per-metric regression thresholds are configurable but do not create wins", () => {
  const baseline = seed.observations.find(
    (item) => item.id === "fresh.solidlil.baseline",
  );
  const after = structuredClone(baseline);
  after.id = "fixture.solidlil.checkpoint";
  after.compiler = exactCompiler("checkpoint");
  artifactForMetric(after, "brotli11").sizes.brotli11 += 1;
  const rows = buildComparisons(
    [baseline, after],
    {
      ...matrix,
      libraries: [matrix.libraries.find((item) => item.id === "solidlil")],
    },
    { maxRegressionBytes: { raw: 0, gzip9: 0, brotli11: 1 } },
  );
  const brotli = rows.find((item) => item.metric === "brotli11");
  assert.equal(brotli.outcome, "regression");
  assert.equal(brotli.gatePassed, true);
  assert.equal(
    rows.find((item) => item.metric === "raw").reason,
    "library has no configured raw objective lane",
  );
});

test("objective lookup selects the matching Marked artifact", () => {
  const published = seed.observations.find(
    (item) => item.id === "published.markedlil",
  );
  assert.equal(artifactForMetric(published, "raw").id, "raw-objective");
  assert.equal(artifactForMetric(published, "gzip9").id, "gzip-objective");
  assert.equal(
    artifactForMetric(published, "brotli11").id,
    "brotli-objective-shipped-esm",
  );
});

test("diagnostic variants cannot shadow comparison observations", () => {
  const markedRows = seed.comparisons.filter((row) => row.library === "markedlil");
  assert.ok(markedRows.every((row) => row.beforeObservation === "fresh.markedlil.baseline"));
  assert.ok(markedRows.every((row) => row.afterObservation === "fresh.markedlil.checkpoint"));
});

test("checkpoint binary identity is tied to the exact checkpoint source", () => {
  const observation = seed.observations.find(
    (item) => item.id === "fresh.solidlil.checkpoint",
  );
  assert.equal(
    observation.compiler.binarySha256,
    "d5e2abee2d3c3ca82a69e262c8dd819c440933f570e62eae4744db2eb021284c",
  );
  assert.equal(
    observation.compiler.primarySourceSha256,
    "607ac880caa60b57011ac2ee0639f4b01c50cde543fffa9d47e710e7284e5684",
  );
  assert.doesNotMatch(stableJson(seed), /a38936b1/u);
});

test("fingerprints fail closed when evidence changes", () => {
  const tampered = structuredClone(seed);
  tampered.observations[0].notes.push("unrecorded mutation");
  assert.throws(() => assertResult(tampered, matrix), /fingerprint is stale/);
});
