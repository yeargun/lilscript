import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
} from "../../../benchmarks/codec-contract.mjs";
import { compilerPath } from "../tooling/compiler-path.mjs";
import { entryBundle, root } from "./project.mjs";

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env,
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function capture(command, args, cwd = root) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    env: process.env,
  });
  return result.status === 0 ? result.stdout.trim() : null;
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

run(process.execPath, [resolve(root, "tooling", "ensure-compiler.mjs")]);
run(process.execPath, [resolve(root, "scripts", "verify-build-modes.mjs")]);
const viteCli = resolve(root, "node_modules", "vite", "bin", "vite.js");
const closureCli = resolve(
  root,
  "node_modules",
  "google-closure-compiler",
  "cli.js",
);
run(process.execPath, [viteCli, "build", "--mode", "solid"]);
run(process.execPath, [viteCli, "build", "--mode", "lilscript"]);
run(process.execPath, [viteCli, "build", "--mode", "lsx-solid"]);
run(process.execPath, [viteCli, "build", "--mode", "lsx-lilscript"]);
run(
  process.execPath,
  [
    resolve(root, "node_modules", "vitest", "vitest.mjs"),
    "run",
    "--config",
    "vitest.lsx.config.js",
  ],
  {
    ...process.env,
    SOLIDLIL_TEST_BUILT_LSX: "1",
  },
);

const generated = resolve(root, "artifacts", "generated");
mkdirSync(generated, { recursive: true });
const bundles = {
  "solid-vite": entryBundle("solid"),
  "lilscript-vite": entryBundle("lilscript"),
  "solid-lsx-vite": entryBundle("lsx-solid"),
  "solidlil-lsx-vite": entryBundle("lsx-lilscript"),
};

for (const [name, source] of Object.entries(bundles)) {
  copyFileSync(source, resolve(generated, `${name}.js`));
  if (name.includes("-lsx-")) continue;
  run(process.execPath, [
    closureCli,
    "--js",
    source,
    "--js_output_file",
    resolve(generated, `${name.replace("-vite", "-closure-advanced")}.js`),
    "--externs",
    resolve(root, "tooling", "closure.externs.js"),
    "--compilation_level",
    "ADVANCED",
    "--language_in",
    "STABLE",
    "--language_out",
    "ECMASCRIPT_NEXT",
    "--warning_level",
    "QUIET",
    "--emit_use_strict=false",
  ]);
}

run(compilerPath(), [
  resolve(root, "apps", "lilscript", "src", "main.lil"),
  "--target",
  "js",
  "--config",
  resolve(root, "config", "closed-world.toml"),
  "-o",
  resolve(generated, "lilscript-compiler.js"),
]);

const packageDefinition = JSON.parse(
  readFileSync(resolve(root, "package.json"), "utf8"),
);
const selectedCompiler = compilerPath();
const compilerVersion =
  capture(selectedCompiler, ["--version"], root) ?? "unreported";
const codecEvidence = canonicalCodecProvenance("SolidLil complete build");
const compilerEvidence = {
  source: process.env.LILSCRIPT_COMPILER
    ? "LILSCRIPT_COMPILER"
    : "repository-release",
  path: relative(root, selectedCompiler),
  sha256: sha256File(selectedCompiler),
  version: compilerVersion,
};
writeFileSync(
  resolve(root, "artifacts", "toolchain.json"),
  `${JSON.stringify(
    {
      node: process.version,
      npm: process.env.npm_config_user_agent ?? null,
      lilscript: compilerVersion,
      compiler: compilerEvidence,
      codecs: codecEvidence,
      solidSourceCommit: capture(
        "git",
        ["rev-parse", "HEAD"],
        resolve(root, "upstream", "solid"),
      ),
      dependencies: packageDefinition.dependencies,
      devDependencies: packageDefinition.devDependencies,
    },
    null,
    2,
  )}\n`,
);

const reportNames = [
  "solid-vite",
  "solid-closure-advanced",
  "lilscript-vite",
  "lilscript-closure-advanced",
  "lilscript-compiler",
  "solid-lsx-vite",
  "solidlil-lsx-vite",
  "solid-core-open",
  "solidlil-core-open",
];
const reportPaths = reportNames.map((name) => resolve(generated, `${name}.js`));
const reportMeasurements = canonicalCodecMeasurementsForFiles(
  reportPaths,
  "SolidLil complete build",
);
const report = Object.fromEntries(
  reportNames.map((name, index) => {
    const measured = reportMeasurements[index];
    return [
      name,
      {
        raw: measured.raw,
        gzip9: measured.gzip,
        brotli11: measured.brotli,
      },
    ];
  }),
);
const artifactEvidence = Object.fromEntries(
  reportNames.map((name, index) => [
    name,
    {
      path: relative(root, reportPaths[index]),
      sha256: sha256File(reportPaths[index]),
    },
  ]),
);
const sizeEvidence = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  objectiveContract: {
    gateMetric: "brotli11",
    matchingArtifactOnly: true,
    crossMetricsAreDiagnostic: ["raw", "gzip9"],
  },
  codecs: codecEvidence,
  compiler: compilerEvidence,
  artifacts: artifactEvidence,
  sizes: report,
};
writeFileSync(
  resolve(root, "artifacts", "size-report.json"),
  `${JSON.stringify(sizeEvidence, null, 2)}\n`,
);

const rows = Object.entries(report)
  .map(
    ([name, value]) =>
      `| ${name} | ${value.brotli11} | ${value.gzip9} | ${value.raw} |`,
  )
  .join("\n");
const sizePairs = [
  ["closed-world Vite app", "solid-vite", "lilscript-vite"],
  [
    "closed-world Closure app",
    "solid-closure-advanced",
    "lilscript-closure-advanced",
  ],
  ["open-world core API", "solid-core-open", "solidlil-core-open"],
  ["closed-world LSX parity fixture", "solid-lsx-vite", "solidlil-lsx-vite"],
];
const sizeFailures = sizePairs
  .filter(
    ([, baselineName, candidateName]) =>
      report[candidateName].brotli11 >= report[baselineName].brotli11,
  )
  .map(
    ([label, baselineName, candidateName]) =>
      `${label}/brotli11: ${report[candidateName].brotli11} >= ${report[baselineName].brotli11}`,
  );
const gateRows = sizePairs
  .map(([label, baselineName, candidateName]) => {
    const status = (metric) =>
      report[candidateName][metric] < report[baselineName][metric]
        ? "pass"
        : "tradeoff";
    return `| ${label} | ${status("brotli11")} | ${status("gzip9")} | ${status("raw")} |`;
  })
  .join("\n");
writeFileSync(
  resolve(root, "artifacts", "size-report.md"),
  `# Bundle size report\n\nBrotli-11 transfer bytes are the primary release gate. Gzip-9 and raw bytes remain explicit diagnostics.\n\n| Comparison | Brotli-11 · primary | Gzip-9 | Raw |\n| --- | --- | --- | --- |\n${gateRows}\n\n| Artifact | Brotli-11 · primary | Gzip-9 | Raw |\n| --- | ---: | ---: | ---: |\n${rows}\n`,
);
console.log(readFileSync(resolve(root, "artifacts", "size-report.md"), "utf8"));
if (sizeFailures.length > 0) {
  throw new Error(`SolidLil size gate failed: ${sizeFailures.join(", ")}`);
}
