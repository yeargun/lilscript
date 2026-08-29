#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertMatrix,
  assertResult,
  canonicalResult,
  sha256,
  stableJson,
} from "./contract.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repository = resolve(here, "../..");
const matrixPath = join(here, "matrix.json");
const seedPath = join(here, "results/seed.json");
const matrixBytes = readFileSync(matrixPath);
const matrix = assertMatrix(JSON.parse(matrixBytes));
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const maxDiagnosticBytes = 128 * 1024;

function usage() {
  return `Usage:
  node comparison/large-libraries/run.mjs [--check]
  node comparison/large-libraries/run.mjs --check-inputs
  node comparison/large-libraries/run.mjs --record-existing [options]
  node comparison/large-libraries/run.mjs --run [options]

Modes:
  --check             Validate the matrix and immutable seed (default; no build)
  --check-inputs      Also archive and hash every pinned sibling input (no build)
  --record-existing   Measure committed artifacts from pinned archives (no compile)
  --run               Build exact compiler revisions and run the long matrix

Options:
  --only IDS          Comma-separated library ids
  --compiler ID       a matrix compiler id, or both for baseline+checkpoint (only with --run)
  --codec PATH        Existing lilscript-codec for --record-existing
  --max-regression M  raw=N,gzip9=N,brotli11=N
  --output PATH       Write canonical JSON instead of stdout
  --keep-temp         Keep the isolated archive directory
  --help              Show this text
`;
}

function parseArguments(argv) {
  const options = {
    mode: "check",
    only: matrix.libraries.map((library) => library.id),
    compiler: "both",
    codec: null,
    output: null,
    keepTemp: false,
    maxRegressionBytes: { ...matrix.regressionPolicy.maxRegressionBytes },
  };
  let explicitMode = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (["--check", "--check-inputs", "--record-existing", "--run"].includes(argument)) {
      if (explicitMode) throw new Error("select exactly one mode");
      options.mode = argument.slice(2);
      explicitMode = true;
    } else if (argument === "--only") {
      options.only = (argv[++index] ?? "").split(",").filter(Boolean);
    } else if (argument === "--compiler") {
      options.compiler = argv[++index];
    } else if (argument === "--codec") {
      options.codec = resolve(argv[++index] ?? "");
    } else if (argument === "--output") {
      options.output = resolve(argv[++index] ?? "");
    } else if (argument === "--max-regression") {
      const entries = (argv[++index] ?? "").split(",");
      for (const entry of entries) {
        const [metric, rawValue] = entry.split("=");
        const value = Number(rawValue);
        if (!(metric in options.maxRegressionBytes) || !Number.isSafeInteger(value) || value < 0) {
          throw new Error(`invalid regression threshold ${entry}`);
        }
        options.maxRegressionBytes[metric] = value;
      }
    } else if (argument === "--keep-temp") {
      options.keepTemp = true;
    } else if (argument === "--help" || argument === "-h") {
      process.stdout.write(usage());
      process.exit(0);
    } else {
      throw new Error(`unknown argument ${argument}`);
    }
  }
  const known = new Set(matrix.libraries.map((library) => library.id));
  if (options.only.length === 0 || options.only.some((id) => !known.has(id))) {
    throw new Error("--only must name one or more matrix library ids");
  }
  const compilerIds = new Set(matrix.compilers.map((compiler) => compiler.id));
  if (options.compiler !== "both" && !compilerIds.has(options.compiler)) {
    throw new Error("--compiler must be a matrix compiler id or both");
  }
  return options;
}

function tail(previous, chunk) {
  const combined = previous + chunk.toString("utf8");
  return combined.length <= maxDiagnosticBytes
    ? combined
    : combined.slice(combined.length - maxDiagnosticBytes);
}

function commandText(program, args) {
  const quote = (value) =>
    /^[A-Za-z0-9_./:=+-]+$/u.test(value)
      ? value
      : `'${value.replaceAll("'", "'\\''")}'`;
  return [program, ...args].map(quote).join(" ");
}

function parseTime(stderr, name) {
  const matches = [...stderr.matchAll(new RegExp(`^${name}\\s+([0-9.]+)$`, "gmu"))];
  if (matches.length === 0) return null;
  return Number(matches.at(-1)[1]) * 1000;
}

async function runTimed(program, args, { cwd, env = {}, timeoutMs }) {
  const hasPosixTime = process.platform !== "win32" && existsSync("/usr/bin/time");
  const wrappedProgram = hasPosixTime ? "/usr/bin/time" : program;
  const wrappedArgs = hasPosixTime ? ["-p", program, ...args] : args;
  const started = performance.now();
  let stdout = "";
  let stderr = "";
  let timedOut = false;
  let hardKillTimer = null;
  const child = spawn(wrappedProgram, wrappedArgs, {
    cwd,
    env: { ...process.env, ...env },
    detached: process.platform !== "win32",
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout.on("data", (chunk) => {
    stdout = tail(stdout, chunk);
  });
  child.stderr.on("data", (chunk) => {
    stderr = tail(stderr, chunk);
  });
  const timer = setTimeout(() => {
    timedOut = true;
    try {
      if (process.platform === "win32") child.kill("SIGTERM");
      else process.kill(-child.pid, "SIGTERM");
    } catch {
      // It exited between the timeout and the signal.
    }
    hardKillTimer = setTimeout(() => {
      try {
        if (process.platform === "win32") child.kill("SIGKILL");
        else process.kill(-child.pid, "SIGKILL");
      } catch {
        // It exited during the grace period.
      }
    }, 5000);
  }, timeoutMs);
  const result = await new Promise((settle, reject) => {
    child.once("error", reject);
    child.once("close", (code, signal) => settle({ code, signal }));
  });
  clearTimeout(timer);
  if (hardKillTimer !== null) clearTimeout(hardKillTimer);
  const measuredWallMs = performance.now() - started;
  return {
    ...result,
    timedOut,
    stdout,
    stderr,
    command: commandText(program, args),
    timing: {
      scope: "command",
      wallMs: hasPosixTime ? parseTime(stderr, "real") : measuredWallMs,
      userCpuMs: hasPosixTime ? parseTime(stderr, "user") : null,
      systemCpuMs: hasPosixTime ? parseTime(stderr, "sys") : null,
      contention: "unknown",
      diagnosticOnly: true,
      unavailableReason: hasPosixTime
        ? null
        : "platform has no /usr/bin/time -p CPU report",
    },
  };
}

function sync(program, args, { cwd = repository, encoding = "utf8" } = {}) {
  const result = spawnSync(program, args, {
    cwd,
    encoding,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${commandText(program, args)} failed: ${String(result.stderr).trim()}`,
    );
  }
  return result.stdout;
}

function safePath(root, path) {
  if (isAbsolute(path)) throw new Error(`matrix path must be relative: ${path}`);
  const result = resolve(root, path);
  const back = relative(root, result);
  if (back.startsWith("..") || isAbsolute(back)) {
    throw new Error(`matrix path escapes archive: ${path}`);
  }
  return result;
}

function hashFile(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function verifyHash(path, expected, label) {
  if (!existsSync(path) || !lstatSync(path).isFile()) {
    throw new Error(`${label} is missing: ${path}`);
  }
  const actual = hashFile(path);
  if (actual !== expected) {
    throw new Error(`${label} digest ${actual} does not match ${expected}`);
  }
}

function resolveLibraryRepository(library) {
  const configured = process.env[library.repositoryEnv];
  return configured
    ? resolve(configured)
    : resolve(repository, library.defaultSibling);
}

function verifyGitObject(gitRepository, revision, expectedTree, label) {
  if (!existsSync(gitRepository)) throw new Error(`${label} repository is missing`);
  const commit = sync(
    "git",
    ["-C", gitRepository, "rev-parse", `${revision}^{commit}`],
  ).trim();
  const tree = sync(
    "git",
    ["-C", gitRepository, "rev-parse", `${revision}^{tree}`],
  ).trim();
  if (commit !== revision || tree !== expectedTree) {
    throw new Error(
      `${label} resolved to commit ${commit}, tree ${tree}; expected ${revision}, ${expectedTree}`,
    );
  }
}

function exportArchive(gitRepository, revision, expectedTree, destination, label) {
  verifyGitObject(gitRepository, revision, expectedTree, label);
  mkdirSync(destination, { recursive: true });
  const archive = join(dirname(destination), `${basename(destination)}.tar`);
  sync("git", [
    "-C",
    gitRepository,
    "archive",
    "--format=tar",
    `--output=${archive}`,
    revision,
  ]);
  sync("tar", ["-xf", archive, "-C", destination]);
  unlinkSync(archive);
}

function sourceRecord(library, { configSha256 = null, configDerivation = null } = {}) {
  return {
    revision: library.revision,
    tree: library.tree,
    packageLockSha256: library.packageLockSha256,
    entrySha256: library.entry.sha256,
    configSha256: configSha256 ?? library.configs[0].sha256,
    configDerivation,
  };
}

function verifyLibraryArchive(root, library) {
  verifyHash(
    safePath(root, "package-lock.json"),
    library.packageLockSha256,
    `${library.id} package lock`,
  );
  verifyHash(
    safePath(root, library.entry.path),
    library.entry.sha256,
    `${library.id} entry`,
  );
  for (const config of library.configs) {
    verifyHash(
      safePath(root, config.path),
      config.sha256,
      `${library.id} ${config.path}`,
    );
  }
}

function verifyCompilerArchive(root, compiler) {
  verifyHash(
    safePath(root, compiler.primarySourcePath),
    compiler.primarySourceSha256,
    `${compiler.id} compiler source`,
  );
}

function verifyCodecArchive(root) {
  verifyHash(
    safePath(root, matrix.codec.sourcePath),
    matrix.codec.sourceSha256,
    "codec source",
  );
}

function removeStaleOutputs(root, library) {
  for (const relativePath of library.build.cleanPaths) {
    const path = safePath(root, relativePath);
    if (!existsSync(path)) continue;
    if (!lstatSync(path).isFile()) {
      throw new Error(`refusing to remove non-file build output ${path}`);
    }
    unlinkSync(path);
  }
}

function deriveArtifact(root, definition, derivedDirectory) {
  const sourcePath = safePath(root, definition.path);
  if (!existsSync(sourcePath) || !lstatSync(sourcePath).isFile()) {
    throw new Error(`build did not emit ${definition.path}`);
  }
  if (definition.derivation.kind === "identity") {
    return { path: sourcePath, relativePath: definition.path };
  }
  const headerPath = safePath(root, definition.derivation.from);
  if (!existsSync(headerPath) || !lstatSync(headerPath).isFile()) {
    throw new Error(`artifact derivation source is missing: ${definition.derivation.from}`);
  }
  const headerSource = readFileSync(headerPath, "utf8");
  const newline = headerSource.indexOf("\n");
  if (newline === -1) throw new Error(`${definition.derivation.from} has no banner line`);
  const bytes =
    headerSource.slice(0, newline + 1) +
    readFileSync(sourcePath, "utf8").trimEnd() +
    "\n";
  mkdirSync(derivedDirectory, { recursive: true });
  const path = join(derivedDirectory, `${definition.id}.mjs`);
  writeFileSync(path, bytes);
  return {
    path,
    relativePath: `${definition.path} (banner from ${definition.derivation.from})`,
  };
}

function validateCodecProvenance(codecs) {
  const expectedGzip = matrix.codec.gzip9;
  const expectedBrotli = matrix.codec.brotli11;
  for (const key of ["encoder", "libraryVersion", "level", "mtime"]) {
    if (codecs.gzip9?.[key] !== expectedGzip[key]) {
      throw new Error(`codec gzip9.${key} does not match the matrix contract`);
    }
  }
  for (const key of [
    "encoder",
    "libraryVersion",
    "quality",
    "lgwin",
    "mode",
  ]) {
    if (codecs.brotli11?.[key] !== expectedBrotli[key]) {
      throw new Error(`codec brotli11.${key} does not match the matrix contract`);
    }
  }
}

function measure(codec, paths) {
  if (!existsSync(codec)) throw new Error(`codec is missing: ${codec}`);
  const output = sync(codec, ["--json", ...paths]);
  const report = JSON.parse(output);
  if (report.schemaVersion !== 1 || report.artifacts?.length !== paths.length) {
    throw new Error("codec returned an unsupported or incomplete report");
  }
  validateCodecProvenance(report.codecs);
  return report;
}

function codecRecord(codec, report, builtFromRevision) {
  return {
    binarySha256: hashFile(codec),
    sourceSha256: matrix.codec.sourceSha256,
    builtFromRevision,
    schemaVersion: report.schemaVersion,
    gzip9: report.codecs.gzip9,
    brotli11: report.codecs.brotli11,
  };
}

function compilerRecord(specification, binary) {
  return {
    role: specification.id,
    revision: specification.revision,
    tree: specification.tree,
    binarySha256: hashFile(binary),
    primarySourceSha256: specification.primarySourceSha256,
    sourceIdentity: `exact git archive ${specification.revision}`,
  };
}

function publishedCompilerRecord() {
  return {
    role: "published-unknown",
    revision: null,
    tree: null,
    binarySha256: null,
    primarySourceSha256: null,
    sourceIdentity: null,
  };
}

function notRunSemantic(summary = "semantic gate was not run") {
  return {
    status: "not-run",
    evidenceClass: "none",
    command: null,
    summary,
  };
}

function aggregateSemantic(artifacts) {
  const statuses = artifacts
    .filter((artifact) => artifact.role === "gate")
    .map((artifact) => artifact.semantic.status);
  if (statuses.every((status) => status === "passed")) {
    return {
      status: "passed",
      evidenceClass: "fresh",
      command: null,
      summary: "every independently gated artifact passed fresh semantics",
    };
  }
  if (statuses.every((status) => status === "failed")) {
    return {
      status: "failed",
      evidenceClass: "fresh",
      command: null,
      summary: "every independently gated artifact failed semantics",
    };
  }
  if (statuses.every((status) => status === "not-run")) {
    return notRunSemantic();
  }
  return {
    status: "partial",
    evidenceClass: "fresh",
    command: null,
    summary: "artifact lanes have different semantic outcomes",
  };
}

function artifactRecords(library, derived, measurements) {
  return library.build.artifacts.map((definition, index) => {
    const config = library.configs.find((item) => item.path === definition.configPath);
    const file = readFileSync(derived[index].path);
    const measured = measurements[index];
    return {
      id: definition.id,
      role: "gate",
      objective: definition.objective,
      gateMetrics: definition.gateMetrics,
      relativePath: derived[index].relativePath,
      configSha256: config.sha256,
      sha256: createHash("sha256").update(file).digest("hex"),
      sizes: {
        raw: measured.raw,
        gzip9: measured.gzip9,
        brotli11: measured.brotli11,
      },
      derivation: {
        kind: definition.derivation.kind,
        from: definition.derivation.from ?? null,
      },
      semantic: notRunSemantic(),
    };
  });
}

function substitutedSemanticArgs(args, root, artifactPath) {
  return args.map((argument) =>
    argument
      .replaceAll("{harness}", here)
      .replaceAll("{project}", root)
      .replaceAll("{artifact}", artifactPath ?? ""),
  );
}

async function runSemantics(root, library, artifacts, derived) {
  const semantic = library.semantic;
  if (semantic.scope === "artifact") {
    for (let index = 0; index < artifacts.length; index += 1) {
      const args = substitutedSemanticArgs(semantic.args, root, derived[index].path);
      const result = await runTimed(semantic.program, args, {
        cwd: root,
        timeoutMs: semantic.timeoutMs,
      });
      artifacts[index].semantic = {
        status: result.code === 0 && !result.timedOut ? "passed" : "failed",
        evidenceClass: "fresh",
        command: result.command,
        summary:
          result.code === 0 && !result.timedOut
            ? result.stdout.trim().split("\n").at(-1) || "semantic command passed"
            : `${result.timedOut ? "timed out" : `exit ${result.code}`}: ${result.stderr.trim().slice(-2000)}`,
      };
    }
  } else {
    const args = substitutedSemanticArgs(semantic.args, root, null);
    const result = await runTimed(semantic.program, args, {
      cwd: root,
      timeoutMs: semantic.timeoutMs,
    });
    const value = {
      status: result.code === 0 && !result.timedOut ? "passed" : "failed",
      evidenceClass: "fresh",
      command: result.command,
      summary:
        result.code === 0 && !result.timedOut
          ? result.stdout.trim().split("\n").at(-1) || "semantic command passed"
          : `${result.timedOut ? "timed out" : `exit ${result.code}`}: ${result.stderr.trim().slice(-2000)}`,
    };
    for (const artifact of artifacts) artifact.semantic = { ...value };
  }
  return aggregateSemantic(artifacts);
}

function failureObservation({ library, compiler, status, result, phase, kind, notes = [] }) {
  const publicCompiler = { ...compiler };
  delete publicCompiler.binaryPath;
  return {
    id: `fresh.${library.id}.${compiler.role}`,
    library: library.id,
    purpose: "comparison",
    evidenceClass: "fresh",
    recordedAt: new Date().toISOString(),
    compiler: publicCompiler,
    source: sourceRecord(library),
    status,
    artifacts: [],
    semantic: notRunSemantic("no artifact was available for semantic testing"),
    timing: result?.timing ?? {
      scope: phase,
      wallMs: null,
      userCpuMs: null,
      systemCpuMs: null,
      contention: "unknown",
      diagnosticOnly: true,
      unavailableReason: "the command did not start",
    },
    failure: {
      phase,
      kind,
      diagnostic: result
        ? `${result.command}\n${result.stderr}`.trim().slice(-8000)
        : "preparation failed before a timed command",
      artifactEmitted: false,
    },
    notes,
  };
}

async function installDependencies(root, library) {
  console.error(`[${library.id}] npm ci`);
  return runTimed(
    process.env.NPM ?? "npm",
    ["ci", "--ignore-scripts", "--no-audit", "--no-fund"],
    { cwd: root, timeoutMs: 600000 },
  );
}

async function runLibrary(root, library, compiler, codec) {
  verifyLibraryArchive(root, library);
  removeStaleOutputs(root, library);
  const install = await installDependencies(root, library);
  if (install.code !== 0 || install.timedOut) {
    return failureObservation({
      library,
      compiler,
      status: install.timedOut ? "timeout" : "compile-error",
      result: install,
      phase: "prepare",
      kind: install.timedOut ? "timeout" : "compile-error",
      notes: ["dependency installation failed; compilation was not attempted"],
    });
  }

  console.error(`[${library.id}/${compiler.role}] ${library.build.program} ${library.build.args.join(" ")}`);
  const build = await runTimed(library.build.program, library.build.args, {
    cwd: root,
    env: {
      ...library.build.environment,
      [library.build.compilerEnv]: compiler.binaryPath,
    },
    timeoutMs: library.build.timeoutMs,
  });
  if (build.code !== 0 || build.timedOut || build.signal !== null) {
    const crash = !build.timedOut && build.signal !== null;
    return failureObservation({
      library,
      compiler,
      status: build.timedOut ? "timeout" : "compile-error",
      result: build,
      phase: "compile",
      kind: build.timedOut ? "timeout" : crash ? "crash" : "compile-error",
    });
  }

  let derived;
  try {
    const derivedDirectory = safePath(root, ".large-library-derived");
    derived = library.build.artifacts.map((definition) =>
      deriveArtifact(root, definition, derivedDirectory),
    );
  } catch (error) {
    return failureObservation({
      library,
      compiler,
      status: "compile-error",
      result: build,
      phase: "measure",
      kind: "compile-error",
      notes: [String(error)],
    });
  }
  const report = measure(codec, derived.map((item) => item.path));
  const artifacts = artifactRecords(library, derived, report.artifacts);
  const semantic = await runSemantics(root, library, artifacts, derived);
  const publicCompiler = { ...compiler };
  delete publicCompiler.binaryPath;
  return {
    id: `fresh.${library.id}.${compiler.role}`,
    library: library.id,
    purpose: "comparison",
    evidenceClass: "fresh",
    recordedAt: new Date().toISOString(),
    compiler: publicCompiler,
    source: sourceRecord(library),
    status: "passed",
    artifacts,
    semantic,
    timing: { ...build.timing, scope: "packaged build command" },
    failure: null,
    notes: ["source was exported from the pinned Git tree into an isolated temporary directory"],
  };
}

async function buildCompiler(tempRoot, specification, { codec = false } = {}) {
  const root = join(tempRoot, `compiler-${specification.id}${codec ? "-codec" : ""}`);
  exportArchive(
    repository,
    specification.revision,
    specification.tree,
    root,
    `compiler ${specification.id}`,
  );
  verifyCompilerArchive(root, specification);
  if (codec) verifyCodecArchive(root);
  const args = ["build", "--release", "--locked", "--bin", codec ? "lilscript-codec" : "lilscript"];
  console.error(`[compiler/${specification.id}] ${process.env.CARGO ?? "cargo"} ${args.join(" ")}`);
  const result = await runTimed(process.env.CARGO ?? "cargo", args, {
    cwd: root,
    env: { CARGO_TARGET_DIR: join(root, "target") },
    timeoutMs: 1800000,
  });
  if (result.code !== 0 || result.timedOut) {
    throw new Error(
      `failed to build exact ${specification.id} ${codec ? "codec" : "compiler"}: ${result.stderr}`,
    );
  }
  const binary = join(
    root,
    "target",
    "release",
    `${codec ? "lilscript-codec" : "lilscript"}${executableSuffix}`,
  );
  if (!existsSync(binary)) throw new Error(`build did not emit ${binary}`);
  chmodSync(binary, 0o755);
  return binary;
}

function existingCodec(option) {
  const candidates = [
    option,
    process.env.LILSCRIPT_CODEC,
    join(repository, "target", "release", `lilscript-codec${executableSuffix}`),
  ].filter(Boolean);
  const codec = candidates.find((candidate) => existsSync(candidate));
  if (!codec) {
    throw new Error("lilscript-codec is missing; pass --codec or set LILSCRIPT_CODEC");
  }
  return resolve(codec);
}

function validateSeed() {
  const result = JSON.parse(readFileSync(seedPath, "utf8"));
  if (result.matrixSha256 !== sha256(matrixBytes)) {
    throw new Error("seed matrix digest is stale");
  }
  assertResult(result, matrix);
  return result;
}

function archiveSelectedInputs(tempRoot, selected) {
  for (const compiler of matrix.compilers) {
    const root = join(tempRoot, `input-compiler-${compiler.id}`);
    exportArchive(
      repository,
      compiler.revision,
      compiler.tree,
      root,
      `compiler ${compiler.id}`,
    );
    verifyCompilerArchive(root, compiler);
    if (compiler.id === matrix.codec.buildFromCompiler) verifyCodecArchive(root);
  }
  for (const library of selected) {
    const root = join(tempRoot, `input-${library.id}`);
    exportArchive(
      resolveLibraryRepository(library),
      library.revision,
      library.tree,
      root,
      library.id,
    );
    verifyLibraryArchive(root, library);
  }
}

function resultEnvelope(observations, codec, options) {
  return canonicalResult(
    {
      schemaVersion: 1,
      format: "lilscript-large-library-observations",
      matrixSha256: sha256(matrixBytes),
      regressionPolicy: {
        semanticStatusRequired: "passed",
        maxRegressionBytes: options.maxRegressionBytes,
      },
      codec,
      observations,
      comparisons: [],
      evidenceFingerprint: "0".repeat(64),
    },
    matrix,
  );
}

function emitResult(result, output) {
  assertResult(result, matrix);
  const bytes = stableJson(result);
  if (output) {
    mkdirSync(dirname(output), { recursive: true });
    writeFileSync(output, bytes);
    console.error(`wrote ${output}`);
  } else {
    process.stdout.write(bytes);
  }
}

async function recordExisting(tempRoot, selected, options) {
  const codec = existingCodec(options.codec);
  const probe = measure(codec, [matrixPath]);
  const observations = [];
  for (const library of selected) {
    const root = join(tempRoot, `published-${library.id}`);
    exportArchive(
      resolveLibraryRepository(library),
      library.revision,
      library.tree,
      root,
      library.id,
    );
    verifyLibraryArchive(root, library);
    const derived = library.build.artifacts.map((definition) =>
      deriveArtifact(root, definition, safePath(root, ".large-library-derived")),
    );
    const report = measure(codec, derived.map((item) => item.path));
    const artifacts = artifactRecords(library, derived, report.artifacts);
    observations.push({
      id: `published.${library.id}.recorded-existing`,
      library: library.id,
      purpose: "published",
      evidenceClass: "published",
      recordedAt: new Date().toISOString(),
      compiler: publishedCompilerRecord(),
      source: sourceRecord(library),
      status: "passed",
      artifacts,
      semantic: notRunSemantic("record-existing intentionally does not install dependencies or run tests"),
      timing: {
        scope: "compile",
        wallMs: null,
        userCpuMs: null,
        systemCpuMs: null,
        contention: "unknown",
        diagnosticOnly: true,
        unavailableReason: "record-existing does not compile",
      },
      failure: null,
      notes: ["artifact was read from the pinned Git archive; no working-tree file was used"],
    });
  }
  emitResult(
    resultEnvelope(observations, codecRecord(codec, probe, null), options),
    options.output,
  );
}

async function runMatrix(tempRoot, selected, options) {
  const checkpoint = matrix.compilers.find((item) => item.id === "checkpoint");
  const codec = await buildCompiler(tempRoot, checkpoint, { codec: true });
  const probe = measure(codec, [matrixPath]);
  const compilerIds =
    options.compiler === "both" ? ["baseline", "checkpoint"] : [options.compiler];
  const observations = [];
  for (const compilerId of compilerIds) {
    const specification = matrix.compilers.find((item) => item.id === compilerId);
    const binary = await buildCompiler(tempRoot, specification);
    const compiler = { ...compilerRecord(specification, binary), binaryPath: binary };
    for (const library of selected) {
      const root = join(tempRoot, `${compilerId}-${library.id}`);
      exportArchive(
        resolveLibraryRepository(library),
        library.revision,
        library.tree,
        root,
        library.id,
      );
      observations.push(await runLibrary(root, library, compiler, codec));
    }
  }
  emitResult(
    resultEnvelope(
      observations,
      codecRecord(codec, probe, checkpoint.revision),
      options,
    ),
    options.output,
  );
}

const options = parseArguments(process.argv.slice(2));
const selected = matrix.libraries.filter((library) => options.only.includes(library.id));

if (options.mode === "check") {
  const seed = validateSeed();
  console.log(
    `large-library evidence valid: ${seed.observations.length} observations, ${seed.comparisons.length} metric rows`,
  );
} else {
  const tempRoot = mkdtempSync(join(tmpdir(), "lilscript-large-libraries-"));
  try {
    if (options.mode === "check-inputs") {
      validateSeed();
      archiveSelectedInputs(tempRoot, selected);
      console.log(`large-library inputs valid: ${selected.map((item) => item.id).join(", ")}`);
    } else if (options.mode === "record-existing") {
      await recordExisting(tempRoot, selected, options);
    } else if (options.mode === "run") {
      await runMatrix(tempRoot, selected, options);
    } else {
      throw new Error(`unsupported mode ${options.mode}`);
    }
  } finally {
    if (options.keepTemp) console.error(`kept ${tempRoot}`);
    else rmSync(tempRoot, { recursive: true, force: true });
  }
}
