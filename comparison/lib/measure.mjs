import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
  requireCanonicalCodecRuntime,
} from "../../benchmarks/codec-contract.mjs";

const [
  app,
  lilscriptRawPath,
  lilscriptGzipPath,
  lilscriptBrotliPath,
  closurePath,
  lilscriptVersion,
  closureVersion,
  compilerPath,
  closureJarPath,
  closureSha256,
  rawConfigPath,
  gzipConfigPath,
  brotliConfigPath,
] = process.argv.slice(2);
if (!brotliConfigPath) throw new Error("missing measurement arguments");
requireCanonicalCodecRuntime("comparison Closure-app hard gate");

const repo = resolve(app, "../../..");

function digest(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function provenanceFile(path) {
  return {
    path: relative(repo, path),
    digest: digest(path),
  };
}

const actualClosureDigest = digest(closureJarPath);
if (actualClosureDigest !== closureSha256) {
  throw new Error(
    `Closure compiler digest mismatch: expected ${closureSha256}, received ${actualClosureDigest}`,
  );
}

function artifact(path, measurement) {
  const source = readFileSync(path);
  const digest = createHash("sha256").update(source).digest("hex");
  if (digest !== measurement.sha256) {
    throw new Error(`${path} changed before canonical codec measurement`);
  }
  return {
    path: basename(path),
    digest,
    sizes: {
      raw: measurement.raw,
      gzip9: measurement.gzip,
      brotli11: measurement.brotli,
    },
  };
}

const paths = [
  lilscriptRawPath,
  lilscriptGzipPath,
  lilscriptBrotliPath,
  closurePath,
];
const measurements = canonicalCodecMeasurementsForFiles(
  paths,
  `${basename(app)} Closure-app artifacts`,
);

const lilscriptArtifacts = {
  raw: artifact(lilscriptRawPath, measurements[0]),
  gzip9: artifact(lilscriptGzipPath, measurements[1]),
  brotli11: artifact(lilscriptBrotliPath, measurements[2]),
};
const closureArtifact = artifact(closurePath, measurements[3]);

const result = {
  schemaVersion: 4,
  app: basename(app),
  lilscriptVersion,
  closureVersion,
  objectiveContract: {
    raw: "raw-config artifact measured as raw UTF-8",
    gzip9: "gzip-config artifact measured as gzip level 9",
    brotli11: "brotli-config artifact measured as Brotli quality 11, lgwin 22",
  },
  lilscript: {
    raw: lilscriptArtifacts.raw.sizes.raw,
    gzip9: lilscriptArtifacts.gzip9.sizes.gzip9,
    brotli11: lilscriptArtifacts.brotli11.sizes.brotli11,
  },
  lilscriptArtifacts,
  closure: closureArtifact.sizes,
  closureArtifact,
  codecs: canonicalCodecProvenance("comparison Closure-app report"),
  toolVersions: {
    lilscript: {
      ...provenanceFile(compilerPath),
      version: `lilscript ${lilscriptVersion}`,
    },
    closure: {
      ...provenanceFile(closureJarPath),
      version: closureVersion,
    },
  },
  provenance: {
    configs: {
      raw: provenanceFile(rawConfigPath),
      gzip: provenanceFile(gzipConfigPath),
      brotli: provenanceFile(brotliConfigPath),
    },
  },
};
const build = join(app, "build");
writeFileSync(
  join(build, "report.json"),
  `${JSON.stringify(result, null, 2)}\n`,
);

const winner = (metric) => {
  const lilscript = result.lilscript[metric];
  const closure = result.closure[metric];
  if (lilscript === closure) return "Tie";
  return lilscript < closure ? "LilScript" : "Closure";
};
const markdown =
  `# ${result.app}\n\n` +
  `LilScript ${lilscriptVersion} vs Closure Compiler ${closureVersion} ADVANCED.\n\n` +
  `Each LilScript column comes from a separate build optimized for that exact ` +
  `objective. Cross-metric sizes of those artifacts are diagnostic only.\n\n` +
  `| Compiler/objective | Raw | Gzip-9 | Brotli-11 |\n` +
  `| --- | ---: | ---: | ---: |\n` +
  `| LilScript objective builds | ${result.lilscript.raw} | ${result.lilscript.gzip9} | ${result.lilscript.brotli11} |\n` +
  `| Closure | ${result.closure.raw} | ${result.closure.gzip9} | ${result.closure.brotli11} |\n\n` +
  `Winners: raw **${winner("raw")}**, gzip **${winner("gzip9")}**, ` +
  `Brotli **${winner("brotli11")}**.\n`;
writeFileSync(join(build, "report.md"), markdown);
process.stdout.write(markdown);
