import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "@babel/parser";

export const projectRoot = resolve(import.meta.dirname, "..");
export const upstreamRoot = resolve(projectRoot, "upstream/vue");
export const scopePath = resolve(projectRoot, "compatibility/scope.json");
export const sourceParityPath = resolve(
  projectRoot,
  "compatibility/source-parity.json",
);

const UPSTREAM_SOURCE = /^packages\/([^/]+)\/src\/(.+)\.(ts|tsx)$/u;
const SHA256 = /^[a-f0-9]{64}$/u;
const legacyCandidates = new Map([
  [
    "src/shared.lil",
    "Package-wide shared implementation does not use the required src/shared/<upstream-relative>.lil layout.",
  ],
  [
    "src/runtime-core/index.lil",
    "Package-wide runtime-core implementation combines responsibilities from multiple upstream modules.",
  ],
  [
    "src/runtime-dom/index.lil",
    "Package-wide runtime-dom implementation combines responsibilities from multiple upstream modules.",
  ],
]);
const allowedHostAdapters = new Map([
  ["src/reactivity/host.js", ["ecmascript-primitives"]],
  ["src/runtime-core/host.js", ["ecmascript-primitives"]],
  ["src/runtime-dom/host.js", ["dom-primitives", "ecmascript-primitives"]],
  [
    "src/compiler-sfc/host.js",
    ["ecmascript-primitives", "filesystem-module-loading"],
  ],
  ["src/compiler-dom/host.js", ["browser-entity-decoding-primitive"]],
  [
    "src/compiler-core/host.js",
    ["parser-primitives", "entity-decoding", "dynamic-function-primitive"],
  ],
]);

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function repositoryPath(path) {
  return relative(projectRoot, path).split(sep).join("/");
}

function runGit(args, { bytes = false } = {}) {
  const result = spawnSync("git", args, {
    cwd: upstreamRoot,
    encoding: bytes ? null : "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(`Unable to run git ${args.join(" ")}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const diagnostic = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString("utf8")
      : result.stderr || result.stdout || "no diagnostic";
    throw new Error(`git ${args.join(" ")} failed: ${diagnostic.trim()}`);
  }
  return result.stdout;
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`Unable to read ${label} at ${path}: ${error.message}`);
  }
}

function walkFiles(root) {
  if (!existsSync(root)) return [];
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = resolve(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(path));
    else files.push(path);
  }
  return files.sort((left, right) => compareText(repositoryPath(left), repositoryPath(right)));
}

export function candidatePathFor(upstreamPath) {
  const match = UPSTREAM_SOURCE.exec(upstreamPath);
  if (!match) {
    throw new Error(`Not a packages/*/src TypeScript implementation path: ${upstreamPath}`);
  }
  return `src/${match[1]}/${match[2]}.lil`;
}

function declarationHasRuntime(node) {
  if (!node) return false;
  if (node.declare === true) return false;
  switch (node.type) {
    case "TSInterfaceDeclaration":
    case "TSTypeAliasDeclaration":
    case "TSDeclareFunction":
    case "TSNamespaceExportDeclaration":
    case "EmptyStatement":
      return false;
    case "TSModuleDeclaration":
      return node.declare !== true && node.global !== true && node.id?.type !== "StringLiteral";
    case "ImportDeclaration":
      if (node.importKind === "type") return false;
      if (node.specifiers.length === 0) return true;
      return node.specifiers.some(
        (specifier) =>
          specifier.importKind !== "type" && specifier.importKind !== "typeof",
      );
    case "ExportNamedDeclaration":
      if (node.exportKind === "type") return false;
      if (node.declaration) return declarationHasRuntime(node.declaration);
      return node.specifiers.some((specifier) => specifier.exportKind !== "type");
    case "ExportAllDeclaration":
      return node.exportKind !== "type";
    default:
      return true;
  }
}

export function classifyTypeScriptSource(source, path = "source.ts") {
  let ast;
  try {
    ast = parse(source, {
      sourceFilename: path,
      sourceType: "module",
      plugins: ["typescript", ...(path.endsWith(".tsx") ? ["jsx"] : [])],
    });
  } catch (error) {
    throw new Error(`Unable to parse upstream TypeScript ${path}: ${error.message}`);
  }
  const typeOnly = ast.program.body.every((node) => !declarationHasRuntime(node));
  return typeOnly ? "type-only" : "algorithm";
}

function stripComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//gu, "")
    .replace(/(^|\s)\/\/[^\n\r]*/gu, "$1")
    .trim();
}

export function inspectCandidateSource(source) {
  const issues = [];
  const substantive = stripComments(source);
  if (substantive === "") issues.push("candidate is empty or comment-only");
  if (/\b(?:todo|fixme|placeholder|not[ _-]?implemented|stub(?:bed)?)\b/iu.test(source)) {
    issues.push("candidate contains a placeholder marker");
  }
  if (
    substantive !== "" &&
    substantive.replace(
      /(?:^|\s)(?:export\s+)?(?:async\s+)?(?:function\s+)?[A-Za-z_$][\w$<>\[\]?| ]*\s+[A-Za-z_$][\w$]*\s*\([^;{}]*\)\s*\{\s*\}\s*;?/gu,
      "",
    ).trim() === ""
  ) {
    issues.push("candidate contains only empty function stubs");
  }

  const modulePattern = /\b(?:from\s*|import\s*\(|require\s*\()\s*["']([^"']+)["']/gu;
  for (const match of source.matchAll(modulePattern)) {
    const specifier = match[1];
    if (
      specifier === "vue" ||
      specifier.startsWith("@vue/") ||
      /(?:^|\/)upstream\/vue(?:\/|$)/u.test(specifier) ||
      /(?:^|\/)node_modules\/(?:vue|@vue)(?:\/|$)/u.test(specifier)
    ) {
      issues.push(`candidate delegates to upstream implementation ${JSON.stringify(specifier)}`);
    }
  }

  if (
    /^(?:\s|\/\*[\s\S]*?\*\/|\/\/[^\n\r]*(?:\r?\n|$))*export\s+(?:\*|\{[\s\S]*?\})\s+from\s+["'][^"']+\.m?js["']\s*;?\s*$/u.test(
      source,
    )
  ) {
    issues.push("candidate is only a JavaScript re-export");
  }
  return [...new Set(issues)];
}

export function detectManyToOneMappings(entries, key = "candidatePath") {
  const targets = new Map();
  for (const entry of entries) {
    const target = entry?.[key];
    if (typeof target !== "string" || target === "") continue;
    const paths = targets.get(target) ?? [];
    paths.push(entry.upstreamPath);
    targets.set(target, paths);
  }
  return [...targets]
    .filter(([, paths]) => paths.length > 1)
    .map(([target, upstreamPaths]) => ({
      target,
      upstreamPaths: [...upstreamPaths].sort(compareText),
    }))
    .sort((left, right) => compareText(left.target, right.target));
}

export function validateDeclarationOnlyHandling(entries, upstreamFiles) {
  const failures = [];
  const validPaths = new Set();
  const seen = new Set();
  if (!Array.isArray(entries)) {
    return {
      failures: ["declarationOnlyHandling must be an array"],
      validPaths,
    };
  }
  const upstreamByPath = new Map(upstreamFiles.map((entry) => [entry.upstreamPath, entry]));
  for (const entry of entries) {
    const path = entry?.upstreamPath;
    if (typeof path !== "string" || path === "") {
      failures.push("declaration-only handling contains an invalid upstreamPath");
      continue;
    }
    if (seen.has(path)) {
      failures.push(`duplicate declaration-only handling for ${path}`);
      continue;
    }
    seen.add(path);
    const upstream = upstreamByPath.get(path);
    if (!upstream) {
      failures.push(`declaration-only handling references unknown source ${path}`);
    } else if (upstream.classification !== "type-only") {
      failures.push(`declaration-only handling is forbidden for algorithm file ${path}`);
    } else if (entry.handling !== "declaration-only") {
      failures.push(`declaration-only handling for ${path} has an invalid handling value`);
    } else if (entry.upstreamSha256 !== upstream.upstreamSha256) {
      failures.push(`declaration-only handling for ${path} has a stale upstream hash`);
    } else if (typeof entry.reason !== "string" || entry.reason.trim().length < 20) {
      failures.push(`declaration-only handling for ${path} needs a specific reason`);
    } else {
      validPaths.add(path);
    }
  }
  return { failures, validPaths };
}

function validateVerifiedMappings(entries, upstreamFiles) {
  const failures = [];
  const validPaths = new Set();
  const seen = new Set();
  if (!Array.isArray(entries)) {
    return { failures: ["verifiedMappings must be an array"], validPaths };
  }
  const upstreamByPath = new Map(upstreamFiles.map((entry) => [entry.upstreamPath, entry]));
  const mappingConflicts = detectManyToOneMappings(entries);
  for (const conflict of mappingConflicts) {
    failures.push(
      `many-to-one verified mapping at ${conflict.target}: ${conflict.upstreamPaths.join(", ")}`,
    );
  }
  for (const entry of entries) {
    const path = entry?.upstreamPath;
    if (typeof path !== "string" || path === "") {
      failures.push("verifiedMappings contains an invalid upstreamPath");
      continue;
    }
    if (seen.has(path)) {
      failures.push(`duplicate verified mapping for ${path}`);
      continue;
    }
    seen.add(path);
    const upstream = upstreamByPath.get(path);
    if (!upstream) {
      failures.push(`verified mapping references unknown source ${path}`);
      continue;
    }
    const expectedCandidate = candidatePathFor(path);
    if (entry.candidatePath !== expectedCandidate) {
      failures.push(`verified mapping for ${path} is not at ${expectedCandidate}`);
    } else if (entry.upstreamSha256 !== upstream.upstreamSha256) {
      failures.push(`verified mapping for ${path} has a stale upstream hash`);
    } else if (!SHA256.test(entry.candidateSha256 ?? "")) {
      failures.push(`verified mapping for ${path} has an invalid candidate hash`);
    } else if (entry.moduleResponsibility !== "preserved") {
      failures.push(`verified mapping for ${path} has not preserved module responsibility`);
    } else if (
      entry.algorithmParity !==
      (upstream.classification === "algorithm" ? "exact" : "not-applicable-type-only")
    ) {
      failures.push(`verified mapping for ${path} has an invalid algorithm-parity claim`);
    } else {
      validPaths.add(path);
    }
  }
  return { failures, mappingConflicts, validPaths };
}

function inspectHostAdapter(path, source) {
  const issues = [];
  for (const match of source.matchAll(
    /\b(?:from\s*|import\s*\(|require\s*\()\s*["']([^"']+)["']/gu,
  )) {
    const specifier = match[1];
    if (
      specifier === "vue" ||
      specifier.startsWith("@vue/") ||
      specifier.includes("upstream/vue")
    ) {
      issues.push(`${path} imports upstream Vue implementation ${JSON.stringify(specifier)}`);
    }
  }
  return issues;
}

function sourceParityConfiguration() {
  if (!existsSync(sourceParityPath)) {
    return { verifiedMappings: [], declarationOnlyHandling: [] };
  }
  const current = readJson(sourceParityPath, "source-parity evidence");
  return {
    verifiedMappings: current.verifiedMappings ?? [],
    declarationOnlyHandling: current.declarationOnlyHandling ?? [],
  };
}

export function serializeSourceParity(report) {
  return `${JSON.stringify(report, null, 2)}\n`;
}

export function buildSourceParity({
  verifiedMappings,
  declarationOnlyHandling,
  verifyClean = true,
} = {}) {
  const scope = readJson(scopePath, "scope manifest");
  const configuration = sourceParityConfiguration();
  const mappingClaims = verifiedMappings ?? configuration.verifiedMappings;
  const declarationClaims =
    declarationOnlyHandling ?? configuration.declarationOnlyHandling;
  const revision = runGit(["rev-parse", "HEAD"]).trim();
  if (revision !== scope.upstream?.revision) {
    throw new Error(
      `Vue checkout revision mismatch: expected ${scope.upstream?.revision}, found ${revision}`,
    );
  }
  const tags = runGit(["tag", "--points-at", "HEAD"])
    .trim()
    .split("\n")
    .filter(Boolean);
  if (!tags.includes(scope.upstream?.tag)) {
    throw new Error(`Vue checkout HEAD is not tagged ${scope.upstream?.tag}`);
  }
  const changes = runGit(["status", "--porcelain", "--untracked-files=no"]).trim();
  if (verifyClean && changes !== "") {
    throw new Error(`Vue checkout is not clean:\n${changes}`);
  }
  const manifest = readJson(resolve(upstreamRoot, "package.json"), "Vue manifest");
  if (manifest.version !== scope.upstream?.version) {
    throw new Error(
      `Vue checkout version mismatch: expected ${scope.upstream?.version}, found ${manifest.version}`,
    );
  }

  const tracked = runGit(["ls-tree", "-r", "--name-only", "-z", "HEAD", "--", "packages"])
    .split("\0")
    .filter(Boolean);
  const trackedManifestPaths = new Set(
    tracked.filter((path) => /^packages\/[^/]+\/package\.json$/u.test(path)),
  );
  const packageNames = new Map();
  for (const path of trackedManifestPaths) {
    const packageManifest = readJson(
      resolve(upstreamRoot, path),
      `package manifest ${path}`,
    );
    packageNames.set(path.split("/")[1], packageManifest.name);
  }
  const upstreamFiles = tracked
    .filter((path) => UPSTREAM_SOURCE.test(path))
    .sort(compareText)
    .map((upstreamPath) => {
      const bytes = readFileSync(resolve(upstreamRoot, upstreamPath));
      const packageDirectory = upstreamPath.split("/")[1];
      if (!packageNames.has(packageDirectory)) {
        throw new Error(`${upstreamPath} belongs to a package without a tracked package.json`);
      }
      return {
        upstreamPath,
        upstreamSha256: sha256(bytes),
        package: packageNames.get(packageDirectory),
        classification: classifyTypeScriptSource(bytes.toString("utf8"), upstreamPath),
        candidatePath: candidatePathFor(upstreamPath),
      };
    });

  const scopedPackages = new Set((scope.packages ?? []).map((entry) => entry.name));
  const discoveredPackages = new Set(upstreamFiles.map((entry) => entry.package));
  const failures = [];
  for (const name of discoveredPackages) {
    if (!scopedPackages.has(name)) failures.push(`upstream source package ${name} is absent from scope`);
  }
  for (const name of scopedPackages) {
    if (!discoveredPackages.has(name)) failures.push(`scoped package ${name} has no tracked source files`);
  }

  const deterministicConflicts = detectManyToOneMappings(upstreamFiles);
  for (const conflict of deterministicConflicts) {
    failures.push(
      `deterministic mapping collision at ${conflict.target}: ${conflict.upstreamPaths.join(", ")}`,
    );
  }
  const declarations = validateDeclarationOnlyHandling(declarationClaims, upstreamFiles);
  failures.push(...declarations.failures);
  const mappings = validateVerifiedMappings(mappingClaims, upstreamFiles);
  failures.push(...mappings.failures);

  const physicalCandidates = [];
  const files = upstreamFiles.map((upstream) => {
    const candidateAbsolutePath = resolve(projectRoot, upstream.candidatePath);
    const legacyReason = legacyCandidates.get(upstream.candidatePath);
    const issues = [];
    let candidate = null;
    if (existsSync(candidateAbsolutePath)) {
      const metadata = lstatSync(candidateAbsolutePath);
      const bytes = readFileSync(candidateAbsolutePath);
      candidate = {
        sha256: sha256(bytes),
        bytes: bytes.length,
      };
      if (!metadata.isFile() || metadata.isSymbolicLink()) {
        issues.push("candidate must be a regular non-symlink file");
      } else {
        const resolved = realpathSync(candidateAbsolutePath);
        const identity = statSync(resolved);
        physicalCandidates.push({
          upstreamPath: upstream.upstreamPath,
          candidatePath: upstream.candidatePath,
          physicalTarget: `${identity.dev}:${identity.ino}`,
        });
      }
      issues.push(...inspectCandidateSource(bytes.toString("utf8")));
    }

    let handling;
    if (legacyReason) handling = "legacy-nonconforming";
    else if (declarations.validPaths.has(upstream.upstreamPath)) {
      if (candidate) issues.push("source has both candidate and declaration-only handling");
      handling = issues.length === 0 ? "declaration-only" : "invalid";
    } else if (!candidate) handling = "missing";
    else if (issues.length > 0) handling = "invalid";
    else if (!mappings.validPaths.has(upstream.upstreamPath)) handling = "unverified";
    else {
      const claim = mappingClaims.find(
        (entry) => entry.upstreamPath === upstream.upstreamPath,
      );
      if (claim.candidateSha256 !== candidate.sha256) {
        issues.push("verified mapping has a stale candidate hash");
        handling = "invalid";
      } else handling = "mapped";
    }
    return { ...upstream, candidate, handling, issues };
  });

  const physicalConflicts = detectManyToOneMappings(
    physicalCandidates.map((entry) => ({
      ...entry,
      candidatePhysicalTarget: entry.physicalTarget,
    })),
    "candidatePhysicalTarget",
  );
  for (const conflict of physicalConflicts) {
    failures.push(
      `multiple upstream files resolve to one physical candidate: ${conflict.upstreamPaths.join(", ")}`,
    );
  }

  const sourceFiles = walkFiles(resolve(projectRoot, "src"));
  const lilFiles = sourceFiles
    .map(repositoryPath)
    .filter((path) => path.endsWith(".lil"));
  const expectedCandidates = new Set(upstreamFiles.map((entry) => entry.candidatePath));
  const legacy = lilFiles
    .filter((path) => legacyCandidates.has(path))
    .map((path) => {
      const bytes = readFileSync(resolve(projectRoot, path));
      return {
        path,
        sha256: sha256(bytes),
        bytes: bytes.length,
        classification: "legacy-nonconforming",
        reason: legacyCandidates.get(path),
        counted: false,
      };
    });
  const unmappedCandidates = lilFiles
    .filter((path) => !expectedCandidates.has(path) && !legacyCandidates.has(path))
    .sort(compareText);
  for (const path of unmappedCandidates) {
    failures.push(`unmapped LilScript algorithm file ${path}`);
  }

  const hostAdapters = [];
  const otherJavaScript = [];
  for (const absolutePath of sourceFiles.filter((path) => /\.m?js$/u.test(path))) {
    const path = repositoryPath(absolutePath);
    const bytes = readFileSync(absolutePath);
    if (allowedHostAdapters.has(path)) {
      const issues = inspectHostAdapter(path, bytes.toString("utf8"));
      hostAdapters.push({
        path,
        sha256: sha256(bytes),
        bytes: bytes.length,
        classification: "primitive-host-adapter",
        primitives: allowedHostAdapters.get(path),
        issues,
      });
      failures.push(...issues);
    } else if (path.endsWith("/build-entry.mjs")) {
      otherJavaScript.push({
        path,
        sha256: sha256(bytes),
        bytes: bytes.length,
        classification: "build-tooling",
      });
    } else {
      otherJavaScript.push({
        path,
        sha256: sha256(bytes),
        bytes: bytes.length,
        classification: "forbidden-javascript-algorithm",
      });
      failures.push(`JavaScript algorithm file is not an allowed primitive adapter: ${path}`);
    }
  }

  const counts = (handling) => files.filter((entry) => entry.handling === handling).length;
  const satisfiedFiles = counts("mapped") + counts("declaration-only");
  const unsatisfiedFiles = files.length - satisfiedFiles;
  if (unsatisfiedFiles > 0) {
    failures.push(`${unsatisfiedFiles} upstream source files lack conforming one-to-one handling`);
  }
  if (legacy.length > 0) {
    failures.push(`${legacy.length} legacy monoliths are nonconforming and are not counted`);
  }

  const auditedGateValues = {
    sourceParityRequired: files.length,
    sourceParitySatisfied: satisfiedFiles,
    sourceParityMissing: counts("missing"),
    sourceParityLegacyMapped: counts("legacy-nonconforming"),
    sourceParityLegacyNonconforming: legacy.length,
  };
  for (const [gate, value] of Object.entries(auditedGateValues)) {
    if (scope.gates?.[gate] !== value) {
      failures.push(`${gate} is ${JSON.stringify(scope.gates?.[gate])}; audited value is ${value}`);
    }
  }
  const auditedStatus = failures.length === 0 ? "passed" : "failed";
  if (scope.gates?.sourceParity !== auditedStatus) {
    failures.push(
      `sourceParity gate is ${JSON.stringify(scope.gates?.sourceParity)}; audited status is ${auditedStatus}`,
    );
  }

  const report = {
    schemaVersion: 1,
    upstream: {
      package: scope.upstream.package,
      version: manifest.version,
      tag: scope.upstream.tag,
      revision,
      tree: runGit(["rev-parse", "HEAD^{tree}"]).trim(),
      clean: changes === "",
    },
    policy: {
      sourcePattern: "^packages/[^/]+/src/.+\\.tsx?$",
      candidatePathRule:
        "packages/<package>/src/<relative>.ts(x) -> src/<package>/<relative>.lil",
      algorithmRule:
        "Each algorithm file requires a non-placeholder LilScript file at its sole deterministic path plus hash-pinned exact-algorithm and preserved-responsibility verification.",
      typeOnlyRule:
        "Type-only files require the same mapped LilScript file or one explicit hash-pinned declaration-only handling record with a specific reason.",
      hostAdapterRule:
        "Only enumerated host.js files may contain JavaScript primitives; adapters cannot satisfy an upstream algorithm mapping or import upstream Vue implementation code.",
    },
    totals: {
      upstreamFiles: files.length,
      algorithmFiles: files.filter((entry) => entry.classification === "algorithm").length,
      typeOnlyFiles: files.filter((entry) => entry.classification === "type-only").length,
      satisfiedFiles,
      mappedFiles: counts("mapped"),
      declarationOnlyFiles: counts("declaration-only"),
      missingFiles: counts("missing"),
      unverifiedFiles: counts("unverified"),
      invalidFiles: counts("invalid"),
      legacyMappedFiles: counts("legacy-nonconforming"),
      legacyNonconformingFiles: legacy.length,
      hostAdapters: hostAdapters.length,
      mappingConflicts:
        deterministicConflicts.length + mappings.mappingConflicts.length,
      physicalManyToOneMappings: physicalConflicts.length,
      unmappedCandidateFiles: unmappedCandidates.length,
    },
    verifiedMappings: [...mappingClaims].sort((left, right) =>
      compareText(left?.upstreamPath ?? "", right?.upstreamPath ?? ""),
    ),
    declarationOnlyHandling: [...declarationClaims].sort((left, right) =>
      compareText(left?.upstreamPath ?? "", right?.upstreamPath ?? ""),
    ),
    hostAdapters,
    supportFiles: otherJavaScript,
    legacyNonconforming: legacy,
    mappingConflicts: {
      deterministic: deterministicConflicts,
      verified: mappings.mappingConflicts,
      physical: physicalConflicts.map(({ target: _target, ...entry }) => entry),
    },
    unmappedCandidates,
    files,
    complete: failures.length === 0 && satisfiedFiles === files.length,
    failures,
  };
  return report;
}

export function auditSourceParity() {
  const report = buildSourceParity();
  writeFileSync(sourceParityPath, serializeSourceParity(report));
  return report;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) {
  const report = auditSourceParity();
  const { totals } = report;
  const summary =
    `Vue ${report.upstream.version} source parity: ${totals.satisfiedFiles}/${totals.upstreamFiles} satisfied ` +
    `(${totals.algorithmFiles} algorithm, ${totals.typeOnlyFiles} type-only, ` +
    `${totals.legacyNonconformingFiles} legacy, ${totals.hostAdapters} host adapters).`;
  if (report.complete) console.log(summary);
  else {
    console.error(summary);
    for (const failure of report.failures) console.error(`- ${failure}`);
    process.exitCode = 1;
  }
}
