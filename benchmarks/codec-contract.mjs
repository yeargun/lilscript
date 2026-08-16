import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const CANONICAL_CODEC_SCHEMA_VERSION = 1;
export const CANONICAL_ZLIB_VERSION = "1.3.1";
export const CANONICAL_BROTLI_VERSION = "1.1.0";

const moduleDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(moduleDirectory, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const configuredCodecBinary = process.env.LILSCRIPT_CODEC;
const codecBinary = configuredCodecBinary
  ? resolve(process.cwd(), configuredCodecBinary)
  : join(repositoryRoot, `target/release/lilscript-codec${executableSuffix}`);
const maximumPathsPerInvocation = 256;
const expectedCodecs = {
  gzip9: {
    encoder: "upstream-stock-zlib-c",
    libraryVersion: CANONICAL_ZLIB_VERSION,
    cargoPackage: "libz-sys",
    cargoPackageVersion: "1.1.24",
    level: 9,
    mtime: 0,
  },
  brotli11: {
    encoder: "official-google-brotli-c",
    libraryVersion: CANONICAL_BROTLI_VERSION,
    cargoPackage: "compu-brotli-sys",
    cargoPackageVersion: "1.1.0",
    quality: 11,
    lgwin: 22,
    mode: "generic",
  },
};

let verifiedBinaryIdentity;
let cachedBinaryFingerprint;

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fail(context, detail) {
  throw new Error(
    `${context} requires the repository's canonical lilscript-codec scorer ` +
      `(stock zlib ${CANONICAL_ZLIB_VERSION} and Google Brotli ${CANONICAL_BROTLI_VERSION}). ${detail}`,
  );
}

export function requirePairedLilscriptOverrides(
  context,
  env = process.env,
) {
  const compilerOverrideSet = Object.hasOwn(env, "LILSCRIPT");
  const codecOverrideSet = Object.hasOwn(env, "LILSCRIPT_CODEC");
  const compilerOverride = env.LILSCRIPT;
  const codecOverride = env.LILSCRIPT_CODEC;
  if (compilerOverrideSet !== codecOverrideSet) {
    throw new Error(
      `${context} requires LILSCRIPT and LILSCRIPT_CODEC overrides to be supplied together so compiler output cannot be measured by an unrelated scorer`,
    );
  }
  if (
    compilerOverrideSet &&
    (compilerOverride?.length === 0 || codecOverride?.length === 0)
  ) {
    throw new Error(
      `${context} requires LILSCRIPT and LILSCRIPT_CODEC overrides to both be non-empty`,
    );
  }
  return { compilerOverride, codecOverride };
}

export function requireExistingLilscriptToolchain(
  context,
  compiler,
  codec,
) {
  if (!existsSync(compiler)) {
    throw new Error(`${context}: LILSCRIPT does not exist: ${compiler}`);
  }
  if (!existsSync(codec)) {
    throw new Error(`${context}: LILSCRIPT_CODEC does not exist: ${codec}`);
  }
}

function binaryIdentity(context) {
  if (!existsSync(codecBinary)) {
    fail(
      context,
      `Missing ${codecBinary}. Run cargo build --release --bin lilscript-codec, ` +
        "or set LILSCRIPT_CODEC to that exact binary.",
    );
  }
  let resolved;
  let stat;
  let bytes;
  try {
    resolved = realpathSync(codecBinary);
    stat = statSync(resolved, { bigint: true });
  } catch (error) {
    fail(context, `Cannot inspect ${codecBinary}: ${error.message}`);
  }
  if (!stat.isFile()) fail(context, `${resolved} is not a regular file.`);
  const fingerprint = {
    absolutePath: resolved,
    bytes: stat.size.toString(),
    modifiedNanoseconds: stat.mtimeNs.toString(),
  };
  if (
    cachedBinaryFingerprint &&
    cachedBinaryFingerprint.absolutePath === fingerprint.absolutePath &&
    cachedBinaryFingerprint.bytes === fingerprint.bytes &&
    cachedBinaryFingerprint.modifiedNanoseconds ===
      fingerprint.modifiedNanoseconds
  ) {
    return cachedBinaryFingerprint.identity;
  }
  try {
    bytes = readFileSync(resolved);
  } catch (error) {
    fail(context, `Cannot hash ${resolved}: ${error.message}`);
  }
  const identity = {
    path: relative(repositoryRoot, resolved) || resolved,
    absolutePath: resolved,
    sha256: sha256(bytes),
    bytes: Number(stat.size),
  };
  cachedBinaryFingerprint = { ...fingerprint, identity };
  return identity;
}

function assertPlainObject(value, label, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(
      context,
      `${label} is not an object; the scorer is stale or incompatible.`,
    );
  }
}

function assertCodecContract(codecs, context) {
  assertPlainObject(codecs, "codecs", context);
  if (JSON.stringify(codecs) !== JSON.stringify(expectedCodecs)) {
    fail(
      context,
      `Codec provenance is stale or incompatible. Expected ${JSON.stringify(expectedCodecs)}, ` +
        `received ${JSON.stringify(codecs)}. Rebuild target/release/lilscript-codec.`,
    );
  }
}

function invokeCodec(paths, context) {
  const result = spawnSync(codecBinary, ["--json", ...paths], {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) {
    fail(context, `Could not execute ${codecBinary}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(
      context,
      `The scorer exited ${result.status}${result.signal ? ` (${result.signal})` : ""}: ` +
        `${(result.stderr || result.stdout || "no diagnostic").trim()}`,
    );
  }
  if (result.stderr !== "") {
    fail(
      context,
      `The scorer wrote unexpected stderr: ${result.stderr.trim()}`,
    );
  }
  let report;
  try {
    report = JSON.parse(result.stdout);
  } catch (error) {
    fail(context, `The scorer returned invalid JSON: ${error.message}`);
  }
  assertPlainObject(report, "report", context);
  if (report.schemaVersion !== CANONICAL_CODEC_SCHEMA_VERSION) {
    fail(
      context,
      `Expected scorer schema ${CANONICAL_CODEC_SCHEMA_VERSION}, received ` +
        `${JSON.stringify(report.schemaVersion)}. Rebuild the scorer.`,
    );
  }
  assertCodecContract(report.codecs, context);
  if (
    !Array.isArray(report.artifacts) ||
    report.artifacts.length !== paths.length
  ) {
    fail(
      context,
      `Expected ${paths.length} artifact measurements, received ` +
        `${Array.isArray(report.artifacts) ? report.artifacts.length : "a non-array"}.`,
    );
  }
  return report;
}

function validateArtifact(measured, path, context) {
  assertPlainObject(measured, `measurement for ${path}`, context);
  if (measured.path !== path) {
    fail(
      context,
      `The scorer reordered or renamed an artifact: expected ${JSON.stringify(path)}, ` +
        `received ${JSON.stringify(measured.path)}.`,
    );
  }
  for (const metric of ["raw", "gzip9", "brotli11"]) {
    if (!Number.isSafeInteger(measured[metric]) || measured[metric] < 0) {
      fail(
        context,
        `${path} has an invalid ${metric} measurement: ${measured[metric]}.`,
      );
    }
  }
  const bytes = readFileSync(path);
  if (measured.raw !== bytes.length) {
    fail(
      context,
      `${path} changed during measurement or the scorer counted the wrong bytes: ` +
        `reported raw=${measured.raw}, observed raw=${bytes.length}.`,
    );
  }
  return {
    path,
    raw: measured.raw,
    gzip: measured.gzip9,
    brotli: measured.brotli11,
    sha256: sha256(bytes),
  };
}

function canonicalCodecMeasurementsForFilesUnchecked(paths, context) {
  if (!Array.isArray(paths) || paths.length === 0) {
    throw new TypeError(
      "canonicalCodecMeasurementsForFiles requires at least one path",
    );
  }
  binaryIdentity(context);
  const normalized = paths.map((path) => {
    if (typeof path !== "string" || path.length === 0) {
      throw new TypeError(
        "canonical codec artifact paths must be non-empty strings",
      );
    }
    return resolve(path);
  });
  const measurements = [];
  for (
    let start = 0;
    start < normalized.length;
    start += maximumPathsPerInvocation
  ) {
    const chunk = normalized.slice(start, start + maximumPathsPerInvocation);
    const report = invokeCodec(chunk, context);
    measurements.push(
      ...report.artifacts.map((artifact, index) =>
        validateArtifact(artifact, chunk[index], context),
      ),
    );
  }
  return measurements;
}

/** Measure exact file bytes in input order with the same native scorer as the compiler. */
export function canonicalCodecMeasurementsForFiles(
  paths,
  context = "benchmark measurement",
) {
  requireCanonicalCodecRuntime(context);
  return canonicalCodecMeasurementsForFilesUnchecked(paths, context);
}

export function canonicalCodecSizesForFile(
  path,
  context = "benchmark measurement",
) {
  const { raw, gzip, brotli } = canonicalCodecMeasurementsForFiles(
    [path],
    context,
  )[0];
  return { raw, gzip, brotli };
}

/** Compatibility helper for in-memory artifacts; exact bytes are written unchanged. */
export function canonicalCodecSizes(value, context = "benchmark measurement") {
  const bytes =
    Buffer.isBuffer(value) || value instanceof Uint8Array
      ? Buffer.from(value)
      : Buffer.from(value);
  const temporaryDirectory = mkdtempSync(join(tmpdir(), "lilscript-codec-"));
  const artifact = join(temporaryDirectory, "artifact.bin");
  try {
    writeFileSync(artifact, bytes);
    return canonicalCodecSizesForFile(artifact, context);
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}

export function requireCanonicalCodecRuntime(
  context = "benchmark measurement",
) {
  const identity = binaryIdentity(context);
  if (verifiedBinaryIdentity?.sha256 === identity.sha256) return;

  // This complete, no-newline artifact distinguishes upstream zlib 1.3.1 from
  // Node 24's separately patched zlib build (79 bytes versus 83 bytes).
  const fixture =
    "let a=[3,4];console.log(42);let b=((a[0]+a[1]|0)+6|0);console.log(b);console.log(b+7|0)";
  const temporaryDirectory = mkdtempSync(
    join(tmpdir(), "lilscript-codec-self-test-"),
  );
  const artifact = join(temporaryDirectory, "fixture.js");
  let measured;
  try {
    writeFileSync(artifact, fixture);
    const row = canonicalCodecMeasurementsForFilesUnchecked(
      [artifact],
      `${context} scorer self-test`,
    )[0];
    measured = { raw: row.raw, gzip: row.gzip, brotli: row.brotli };
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
  if (measured.raw !== 87 || measured.gzip !== 79 || measured.brotli !== 78) {
    fail(
      context,
      `The scorer failed its exact-byte fixture: expected raw=87/gzip9=79/brotli11=78, ` +
        `received raw=${measured.raw}/gzip9=${measured.gzip}/brotli11=${measured.brotli}.`,
    );
  }
  verifiedBinaryIdentity = identity;
}

export function canonicalCodecProvenance(context = "benchmark measurement") {
  requireCanonicalCodecRuntime(context);
  const { path, sha256: digest, bytes } = verifiedBinaryIdentity;
  return {
    implementation: "lilscript-codec",
    schemaVersion: CANONICAL_CODEC_SCHEMA_VERSION,
    scorer: { path, sha256: digest, bytes },
    gzip9: { ...expectedCodecs.gzip9 },
    brotli11: { ...expectedCodecs.brotli11 },
    nodeCodecsAreDiagnosticOnly: true,
  };
}
