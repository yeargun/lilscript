import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve, sep } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  CANONICAL_BROTLI_VERSION,
  CANONICAL_CODEC_SCHEMA_VERSION,
  CANONICAL_ZLIB_VERSION,
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
  canonicalCodecSizes,
  requireCanonicalCodecRuntime,
  requireExistingLilscriptToolchain,
  requirePairedLilscriptOverrides,
} from "./codec-contract.mjs";

const fixtures = [
  ["", { raw: 0, gzip: 20, brotli: 1 }],
  ["a", { raw: 1, gzip: 21, brotli: 5 }],
  [
    'let q=[1,2,3,4,5];console.log(q==q);q.reverse();console.log(q.join("-"))',
    { raw: 72, gzip: 80, brotli: 68 },
  ],
  [
    "var Z=[1,2,3,4,5];console.log(Z==Z),Z.reverse(),console.log(Z.join('-'))",
    { raw: 72, gzip: 81, brotli: 76 },
  ],
];

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("compiler-producing runners require paired tool overrides", () => {
  assert.deepEqual(requirePairedLilscriptOverrides("pair test", {}), {
    compilerOverride: undefined,
    codecOverride: undefined,
  });
  assert.deepEqual(
    requirePairedLilscriptOverrides("pair test", {
      LILSCRIPT: "/compiler",
      LILSCRIPT_CODEC: "/codec",
    }),
    { compilerOverride: "/compiler", codecOverride: "/codec" },
  );
  assert.throws(
    () =>
      requirePairedLilscriptOverrides("pair test", {
        LILSCRIPT: "/compiler",
      }),
    /pair test requires LILSCRIPT and LILSCRIPT_CODEC overrides to be supplied together/u,
  );
  assert.throws(
    () =>
      requirePairedLilscriptOverrides("pair test", {
        LILSCRIPT_CODEC: "/codec",
      }),
    /pair test requires LILSCRIPT and LILSCRIPT_CODEC overrides to be supplied together/u,
  );
  for (const env of [
    { LILSCRIPT: "", LILSCRIPT_CODEC: "" },
    { LILSCRIPT: "/compiler", LILSCRIPT_CODEC: "" },
    { LILSCRIPT: "", LILSCRIPT_CODEC: "/codec" },
  ]) {
    assert.throws(
      () => requirePairedLilscriptOverrides("pair test", env),
      /both be non-empty/u,
    );
  }
});

test("an explicit compiler/scorer pair fails closed when one path is missing", () => {
  const missingCodec = join(
    tmpdir(),
    `definitely-missing-lilscript-codec-${process.pid}-${Date.now()}`,
  );
  assert.doesNotThrow(() =>
    requireExistingLilscriptToolchain(
      "path test",
      process.execPath,
      process.execPath,
    ),
  );
  assert.throws(
    () =>
      requireExistingLilscriptToolchain(
        "path test",
        process.execPath,
        missingCodec,
      ),
    /path test: LILSCRIPT_CODEC does not exist:/u,
  );
});

function runnerFiles(root) {
  const ignoredDirectories = new Set([
    ".git",
    "artifacts",
    "build",
    "node_modules",
    "public",
    "target",
    "upstream",
  ]);
  const files = [];
  const visit = (directory) => {
    const relativeDirectory = relative(root, directory);
    if (
      relativeDirectory === join("docs", "knowledge", "research") ||
      relativeDirectory.startsWith(
        `${join("docs", "knowledge", "research")}${sep}`,
      ) ||
      relativeDirectory === join("benchmarks", "popular", "apps", "monaco") ||
      relativeDirectory.startsWith(
        `${join("benchmarks", "popular", "apps", "monaco")}${sep}`,
      )
    ) {
      return;
    }
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!ignoredDirectories.has(entry.name))
          visit(join(directory, entry.name));
      } else if (
        /\.(?:cjs|js|mjs|sh)$/u.test(entry.name) ||
        entry.name === "package.json"
      ) {
        const path = join(directory, entry.name);
        if (
          relative(root, path) !==
          join("benchmarks", "popular", "monaco-layers", "serve-ide.mjs")
        ) {
          files.push(path);
        }
      }
    }
  };
  visit(root);
  return files;
}

test("canonical codec fixtures match the compiler objective contract", () => {
  requireCanonicalCodecRuntime("codec fixture test");
  for (const [source, expected] of fixtures) {
    assert.deepEqual(
      canonicalCodecSizes(source),
      expected,
      source || "empty input",
    );
  }
});

test("batch measurement preserves exact bytes, path order, and duplicates", () => {
  const directory = mkdtempSync(join(tmpdir(), "lilscript-codec-test-"));
  try {
    const first = join(directory, "first.js");
    const second = join(directory, "second.js");
    writeFileSync(first, Buffer.from([0x61, 0x0a]));
    writeFileSync(second, Buffer.from([0x61]));
    const rows = canonicalCodecMeasurementsForFiles([second, first, second]);
    assert.deepEqual(
      rows.map(({ path, raw }) => [path, raw]),
      [
        [second, 1],
        [first, 2],
        [second, 1],
      ],
    );
    assert.equal(rows[0].sha256, rows[2].sha256);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("codec provenance identifies the pinned scorer and both static libraries", () => {
  const provenance = canonicalCodecProvenance();
  assert.equal(provenance.implementation, "lilscript-codec");
  assert.equal(provenance.schemaVersion, CANONICAL_CODEC_SCHEMA_VERSION);
  assert.match(provenance.scorer.path, /lilscript-codec(?:\.exe)?$/u);
  assert.match(provenance.scorer.sha256, /^[0-9a-f]{64}$/u);
  assert.equal(provenance.gzip9.libraryVersion, CANONICAL_ZLIB_VERSION);
  assert.equal(provenance.gzip9.encoder, "upstream-stock-zlib-c");
  assert.equal(provenance.brotli11.libraryVersion, CANONICAL_BROTLI_VERSION);
  assert.equal(provenance.brotli11.encoder, "official-google-brotli-c");
  assert.equal(provenance.nodeCodecsAreDiagnosticOnly, true);
});

test("cargo forces the bundled zlib scorer on Windows", () => {
  const cargoConfig = readFileSync(
    join(repositoryRoot, ".cargo", "config.toml"),
    "utf8",
  );
  assert.match(cargoConfig, /^LIBZ_SYS_STATIC\s*=\s*\{[^\n]*force\s*=\s*true/mu);
  assert.match(cargoConfig, /^VCPKGRS_NO_ZLIB\s*=\s*\{[^\n]*force\s*=\s*true/mu);
  assert.match(cargoConfig, /^ZLIB_NO_VCPKG\s*=\s*\{[^\n]*force\s*=\s*true/mu);
});

test("benchmark and publication runners use only the canonical codec wrapper", () => {
  const forbiddenSourcePatterns = [
    /(?:from\s+|import\s*\(|require\s*\()\s*["'](?:(?:node:)?zlib(?:\/[^"']*)?|pako|fflate)["']/u,
    /\b(?:CompressionStream|brotliCompressSync|createBrotliCompress|createGzip|gzipSync)\b/u,
    /\b(?:execFile|execFileSync|spawn|spawnSync)\s*\(\s*["'`](?:[^"'`/]*\/)?(?:brotli|gzip)(?:\.exe)?["'`]/u,
  ];
  const offenders = runnerFiles(repositoryRoot)
    .filter((path) => path !== fileURLToPath(import.meta.url))
    .filter((path) => {
      const source = readFileSync(path, "utf8");
      return (
        forbiddenSourcePatterns.some((pattern) => pattern.test(source)) ||
        (path.endsWith(".sh") &&
          /(?:^|[;&|]\s*)(?:brotli|gzip)(?:\s|$)/mu.test(source)) ||
        (path.endsWith("package.json") &&
          /["'](?:pako|fflate)["']\s*:/u.test(source))
      );
    })
    .map((path) => relative(repositoryRoot, path));
  assert.deepEqual(
    offenders,
    [],
    "size evidence must use benchmarks/codec-contract.mjs and lilscript-codec; direct APIs, packages, and compressor subprocesses are forbidden",
  );
});

function isolatedProbe(codecPath) {
  return spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "--eval",
      `import { requireCanonicalCodecRuntime } from ${JSON.stringify(import.meta.url.replace(/\.test\.mjs$/u, ".mjs"))}; requireCanonicalCodecRuntime("isolated test");`,
    ],
    {
      encoding: "utf8",
      env: { ...process.env, LILSCRIPT_CODEC: codecPath },
    },
  );
}

function isolatedMeasurementProbe(codecPath) {
  return spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "--eval",
      `import { canonicalCodecSizes } from ${JSON.stringify(import.meta.url.replace(/\.test\.mjs$/u, ".mjs"))}; canonicalCodecSizes("probe", "isolated measurement");`,
    ],
    {
      encoding: "utf8",
      env: { ...process.env, LILSCRIPT_CODEC: codecPath },
    },
  );
}

test("a missing scorer fails clearly instead of falling back to Node zlib", () => {
  const result = isolatedProbe(
    join(tmpdir(), "definitely-missing-lilscript-codec"),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Missing .*lilscript-codec/u);
  assert.match(result.stderr, /cargo build --release --bin lilscript-codec/u);
});

test(
  "a stale scorer schema fails closed",
  { skip: process.platform === "win32" },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "lilscript-codec-stale-"));
    try {
      const fake = join(directory, "lilscript-codec");
      writeFileSync(
        fake,
        '#!/bin/sh\nprintf \'%s\\n\' \'{"schemaVersion":0,"codecs":{},"artifacts":[]}\'\n',
      );
      chmodSync(fake, 0o755);
      const result = isolatedProbe(fake);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /Expected scorer schema 1/u);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  },
);

test(
  "every public measurement self-tests a metadata-compatible scorer",
  { skip: process.platform === "win32" },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "lilscript-codec-wrong-"));
    try {
      const fake = join(directory, "lilscript-codec");
      writeFileSync(
        fake,
        `#!/usr/bin/env node
const { statSync } = require("node:fs");
const paths = process.argv.slice(3);
const codecs = ${JSON.stringify({
          gzip9: {
            encoder: "upstream-stock-zlib-c",
            libraryVersion: "1.3.1",
            cargoPackage: "libz-sys",
            cargoPackageVersion: "1.1.24",
            level: 9,
            mtime: 0,
          },
          brotli11: {
            encoder: "official-google-brotli-c",
            libraryVersion: "1.1.0",
            cargoPackage: "compu-brotli-sys",
            cargoPackageVersion: "1.1.0",
            quality: 11,
            lgwin: 22,
            mode: "generic",
          },
        })};
process.stdout.write(JSON.stringify({schemaVersion:1,codecs,artifacts:paths.map(path=>({path,raw:statSync(path).size,gzip9:0,brotli11:0}))}));
`,
      );
      chmodSync(fake, 0o755);
      const result = isolatedMeasurementProbe(fake);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /scorer failed its exact-byte fixture/u);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  },
);
