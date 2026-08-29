import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  buildSourceParity,
  candidatePathFor,
  classifyTypeScriptSource,
  detectManyToOneMappings,
  inspectCandidateSource,
  scopePath,
  serializeSourceParity,
  sourceParityPath,
  validateDeclarationOnlyHandling,
} from "../scripts/audit-source-parity.mjs";

const evidence = JSON.parse(readFileSync(sourceParityPath, "utf8"));
const scope = JSON.parse(readFileSync(scopePath, "utf8"));

test("source-parity evidence exactly matches the pinned Git tree and candidate tree", () => {
  const live = buildSourceParity();
  assert.equal(serializeSourceParity(live), readFileSync(sourceParityPath, "utf8"));
  assert.equal(live.upstream.version, "3.5.42");
  assert.equal(live.upstream.tag, "v3.5.42");
  assert.equal(
    live.upstream.revision,
    "d63616ca17de965ed32dcb449a4c5cd9982f15d2",
  );
  assert.equal(live.upstream.clean, true);
  assert.deepEqual(live.totals, {
    upstreamFiles: 234,
    algorithmFiles: 231,
    typeOnlyFiles: 3,
    satisfiedFiles: 92,
    mappedFiles: 91,
    declarationOnlyFiles: 1,
    missingFiles: 140,
    unverifiedFiles: 0,
    invalidFiles: 0,
    legacyMappedFiles: 2,
    legacyNonconformingFiles: 2,
    hostAdapters: 5,
    mappingConflicts: 0,
    physicalManyToOneMappings: 0,
    unmappedCandidateFiles: 0,
  });
  assert.equal(live.complete, false);
  assert.deepEqual(
    {
      sourceParityRequired: scope.gates.sourceParityRequired,
      sourceParitySatisfied: scope.gates.sourceParitySatisfied,
      sourceParityMissing: scope.gates.sourceParityMissing,
      sourceParityLegacyMapped: scope.gates.sourceParityLegacyMapped,
      sourceParityLegacyNonconforming:
        scope.gates.sourceParityLegacyNonconforming,
      sourceParity: scope.gates.sourceParity,
    },
    {
      sourceParityRequired: live.totals.upstreamFiles,
      sourceParitySatisfied: live.totals.satisfiedFiles,
      sourceParityMissing: live.totals.missingFiles,
      sourceParityLegacyMapped: live.totals.legacyMappedFiles,
      sourceParityLegacyNonconforming:
        live.totals.legacyNonconformingFiles,
      sourceParity: "failed",
    },
  );
  assert.equal(live.files.length, 234);
  assert.equal(new Set(live.files.map((entry) => entry.upstreamPath)).size, 234);
  assert.equal(new Set(live.files.map((entry) => entry.candidatePath)).size, 234);
  assert.ok(
    live.files.every(
      (entry) =>
        /^[a-f0-9]{64}$/u.test(entry.upstreamSha256) &&
        entry.candidatePath === candidatePathFor(entry.upstreamPath),
    ),
  );
  const existingDeterministicCandidates = live.files.filter((entry) => entry.candidate);
  assert.equal(existingDeterministicCandidates.length, 93);
  assert.ok(
    existingDeterministicCandidates.every((entry) =>
      /^[a-f0-9]{64}$/u.test(entry.candidate.sha256),
    ),
  );
});

test("type-only files require explicit strict handling", () => {
  const typeOnly = evidence.files
    .filter((entry) => entry.classification === "type-only")
    .map((entry) => entry.upstreamPath);
  assert.deepEqual(typeOnly, [
    "packages/compiler-sfc/src/shims.d.ts",
    "packages/runtime-dom/src/jsx.ts",
    "packages/shared/src/typeUtils.ts",
  ]);
  assert.deepEqual(
    Object.fromEntries(
      evidence.files
        .filter((entry) => entry.classification === "type-only")
        .map((entry) => [entry.upstreamPath, entry.handling]),
    ),
    {
      "packages/compiler-sfc/src/shims.d.ts": "missing",
      "packages/runtime-dom/src/jsx.ts": "missing",
      "packages/shared/src/typeUtils.ts": "declaration-only",
    },
  );

  const source = {
    upstreamPath: "packages/example/src/types.ts",
    upstreamSha256: "a".repeat(64),
    classification: "type-only",
  };
  const valid = validateDeclarationOnlyHandling(
    [
      {
        upstreamPath: source.upstreamPath,
        upstreamSha256: source.upstreamSha256,
        handling: "declaration-only",
        reason: "This module contributes declarations and emits no runtime code.",
      },
    ],
    [source],
  );
  assert.deepEqual([...valid.validPaths], [source.upstreamPath]);
  assert.deepEqual(valid.failures, []);

  const rejected = validateDeclarationOnlyHandling(
    [
      {
        upstreamPath: source.upstreamPath,
        upstreamSha256: "b".repeat(64),
        handling: "declaration-only",
        reason: "stale",
      },
      {
        upstreamPath: source.upstreamPath,
        upstreamSha256: source.upstreamSha256,
        handling: "declaration-only",
        reason: "duplicate declaration handling must be rejected",
      },
    ],
    [source],
  );
  assert.equal(rejected.validPaths.size, 0);
  assert.ok(rejected.failures.some((failure) => failure.includes("stale upstream hash")));
  assert.ok(rejected.failures.some((failure) => failure.includes("duplicate")));
});

test("TypeScript classification is conservative about emitted modules", () => {
  assert.equal(
    classifyTypeScriptSource(
      "import type { X } from './x'\nexport interface Y { value: X }\nexport type Z = Y",
    ),
    "type-only",
  );
  assert.equal(classifyTypeScriptSource("export enum Value { A }"), "algorithm");
  assert.equal(
    classifyTypeScriptSource("export { value } from './implementation'"),
    "algorithm",
  );
});

test("deterministic paths expose duplicate and many-to-one mappings", () => {
  assert.equal(
    candidatePathFor("packages/runtime-core/src/helpers/renderList.ts"),
    "src/runtime-core/helpers/renderList.lil",
  );
  assert.equal(
    candidatePathFor("packages/example/src/view.tsx"),
    "src/example/view.lil",
  );
  const collisions = detectManyToOneMappings([
    { upstreamPath: "packages/example/src/view.ts", candidatePath: "src/example/view.lil" },
    { upstreamPath: "packages/example/src/view.tsx", candidatePath: "src/example/view.lil" },
  ]);
  assert.deepEqual(collisions, [
    {
      target: "src/example/view.lil",
      upstreamPaths: [
        "packages/example/src/view.ts",
        "packages/example/src/view.tsx",
      ],
    },
  ]);
});

test("placeholder and upstream JavaScript delegation checks fail closed", () => {
  assert.deepEqual(inspectCandidateSource("// comments only\n"), [
    "candidate is empty or comment-only",
  ]);
  assert.ok(inspectCandidateSource("// TODO: port this\nexport void run() {}\n").length >= 1);
  assert.ok(inspectCandidateSource("export void run() {}\n").includes(
    "candidate contains only empty function stubs",
  ));
  const delegated = inspectCandidateSource(
    'export * from "../upstream/vue/packages/reactivity/index.js";\n',
  );
  assert.ok(delegated.some((issue) => issue.includes("delegates to upstream")));
  assert.ok(delegated.includes("candidate is only a JavaScript re-export"));
});

test("legacy monoliths and primitive adapters are separate and never count", () => {
  const legacy = new Map(
    evidence.legacyNonconforming.map((entry) => [entry.path, entry]),
  );
  for (const path of ["src/runtime-core/index.lil", "src/runtime-dom/index.lil"]) {
    assert.equal(legacy.get(path)?.classification, "legacy-nonconforming");
    assert.equal(legacy.get(path)?.counted, false);
    assert.match(legacy.get(path)?.sha256 ?? "", /^[a-f0-9]{64}$/u);
  }
  assert.ok(
    evidence.hostAdapters.every(
      (entry) =>
        entry.classification === "primitive-host-adapter" &&
        entry.issues.length === 0 &&
        /^[a-f0-9]{64}$/u.test(entry.sha256),
    ),
  );
  assert.equal(legacy.has("src/compiler-core/index.lil"), false);
  assert.equal(evidence.verifiedMappings.length, 91);
  assert.equal(evidence.declarationOnlyHandling.length, 1);
});

test("compiler-core index is a verified declaration-free barrel", () => {
  const source = readFileSync(
    new URL("../src/compiler-core/index.lil", import.meta.url),
    "utf8",
  );
  const statementsRemoved = source
    .replace(/import\s*\{[\s\S]*?\}\s*from\s*"[^"]+"\s*;/gu, "")
    .replace(/export\s*\{[\s\S]*?\}\s*;/gu, "")
    .trim();
  assert.equal(statementsRemoved, "");
  const bindings = pattern =>
    [...source.matchAll(pattern)]
      .flatMap(match => match[1].split(","))
      .map(binding => binding.trim().split(/\s+as\s+/u).at(-1))
      .sort();
  const imports = bindings(/import\s*\{([\s\S]*?)\}\s*from\s*"[^"]+"\s*;/gu);
  const exports = bindings(/export\s*\{([\s\S]*?)\}\s*;/gu);
  assert.equal(imports.length, 158);
  assert.deepEqual(exports, imports);

  const compilerMappings = evidence.verifiedMappings.filter(entry =>
    entry.upstreamPath.startsWith("packages/compiler-core/src/"),
  );
  assert.equal(compilerMappings.length, 30);
  assert.equal(
    compilerMappings.filter(
      entry => entry.upstreamPath !== "packages/compiler-core/src/index.ts",
    ).length,
    29,
  );
  assert.equal(
    evidence.files.find(
      entry => entry.upstreamPath === "packages/compiler-core/src/index.ts",
    )?.handling,
    "mapped",
  );
});

test("compiler-dom has all sixteen source files verified", () => {
  const mapped = evidence.verifiedMappings
    .filter(entry => entry.upstreamPath.startsWith("packages/compiler-dom/src/"))
    .map(entry => entry.upstreamPath);
  assert.deepEqual(mapped, [
    "packages/compiler-dom/src/decodeHtmlBrowser.ts",
    "packages/compiler-dom/src/errors.ts",
    "packages/compiler-dom/src/htmlNesting.ts",
    "packages/compiler-dom/src/index.ts",
    "packages/compiler-dom/src/parserOptions.ts",
    "packages/compiler-dom/src/runtimeHelpers.ts",
    "packages/compiler-dom/src/transforms/Transition.ts",
    "packages/compiler-dom/src/transforms/ignoreSideEffectTags.ts",
    "packages/compiler-dom/src/transforms/stringifyStatic.ts",
    "packages/compiler-dom/src/transforms/transformStyle.ts",
    "packages/compiler-dom/src/transforms/vHtml.ts",
    "packages/compiler-dom/src/transforms/vModel.ts",
    "packages/compiler-dom/src/transforms/vOn.ts",
    "packages/compiler-dom/src/transforms/vShow.ts",
    "packages/compiler-dom/src/transforms/vText.ts",
    "packages/compiler-dom/src/transforms/validateHtmlNesting.ts",
  ]);

  const remaining = evidence.files.filter(
    entry =>
      entry.upstreamPath.startsWith("packages/compiler-dom/src/") &&
      !mapped.includes(entry.upstreamPath),
  );
  assert.equal(remaining.length, 0);
  assert.equal(remaining.filter(entry => entry.handling === "unverified").length, 0);
  assert.deepEqual(remaining, []);
});

test("compiler-ssr has all seventeen owner modules verified", () => {
  const mapped = evidence.verifiedMappings
    .filter(entry => entry.upstreamPath.startsWith("packages/compiler-ssr/src/"))
    .map(entry => entry.upstreamPath);
  assert.deepEqual(mapped, [
    "packages/compiler-ssr/src/errors.ts",
    "packages/compiler-ssr/src/index.ts",
    "packages/compiler-ssr/src/runtimeHelpers.ts",
    "packages/compiler-ssr/src/ssrCodegenTransform.ts",
    "packages/compiler-ssr/src/transforms/ssrInjectCssVars.ts",
    "packages/compiler-ssr/src/transforms/ssrInjectFallthroughAttrs.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformComponent.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformElement.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformSlotOutlet.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformSuspense.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformTeleport.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformTransition.ts",
    "packages/compiler-ssr/src/transforms/ssrTransformTransitionGroup.ts",
    "packages/compiler-ssr/src/transforms/ssrVFor.ts",
    "packages/compiler-ssr/src/transforms/ssrVIf.ts",
    "packages/compiler-ssr/src/transforms/ssrVModel.ts",
    "packages/compiler-ssr/src/transforms/ssrVShow.ts",
  ]);
  assert.equal(
    evidence.files.filter(
      entry => entry.upstreamPath.startsWith("packages/compiler-ssr/src/") &&
        entry.handling !== "mapped",
    ).length,
    0,
  );
});
