#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import { minify as terserMinify } from "terser";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const portRoot = join(labRoot, "ports/jquery");
const outputRoot = join(labRoot, "build/jquery-config-audit");
mkdirSync(outputRoot, { recursive: true });

const requestedVariants = new Set(process.argv.slice(2));
const variants = [
  { name: "current" },
  { name: "no-string-pool", poolStrings: false },
  { name: "internal-properties", properties: true },
  { name: "positional-aggregates", properties: true, publicAggregateAbi: "positional" },
  { name: "mangled-exports", properties: true, exports: true },
  { name: "comma-conditionals", poolStrings: false, properties: true, optimizations: ["conditional-expression-variants", "comma-expression-variants", "entropy-cross-scope-reuse", "entropy-property-assignment"] },
  { name: "no-inlining", optimization: { inlining: false } },
  { name: "no-closure-inlining", optimization: { inline_closure_factories: false } },
  { name: "no-scalar-replacement", optimization: { scalar_replacement: false } },
  { name: "no-function-folding", optimization: { identical_function_folding: false } },
  { name: "no-function-subsumption", optimization: { function_subsumption: false } },
  {
    name: "plain-ir",
    optimization: {
      inlining: false,
      inline_closure_factories: false,
      constant_parameter_specialization: false,
      call_site_specialization: false,
      capture_signature_cloning: false,
      scalar_replacement: false,
      identical_function_folding: false,
      function_subsumption: false,
      parameterized_function_merging: false,
    },
  },
  { name: "unstable-locals", stableLocalNames: false },
  { name: "no-reserve", localNameReserve: 0 },
  { name: "function-spelling", functionSpelling: "function" },
  { name: "no-number-pool", poolNumericLiterals: false },
  { name: "balanced", priority: "balanced" },
  { name: "realistic-performance", priority: "realistic-performance-first" },
  { name: "performance", priority: "performance-first" },
  { name: "readable", identifiers: false, poolStrings: false },
  {
    name: "lean-debug-names",
    identifiers: false,
    poolStrings: false,
    properties: true,
    priority: "balanced",
  },
  { name: "lean", poolStrings: false, properties: true },
  { name: "lean-balanced", poolStrings: false, properties: true, priority: "balanced" },
  {
    name: "lean-inline-24",
    poolStrings: false,
    properties: true,
    priority: "balanced",
    inlineInstructionLimit: 24,
    inlineControlFlowLimit: 60,
    maxInlineGrowth: 16,
  },
  {
    name: "lean-inline-48",
    poolStrings: false,
    properties: true,
    priority: "balanced",
    inlineInstructionLimit: 48,
    inlineControlFlowLimit: 120,
    maxInlineGrowth: 32,
  },
  {
    name: "lean-inline-96",
    poolStrings: false,
    properties: true,
    priority: "balanced",
    inlineInstructionLimit: 96,
    inlineControlFlowLimit: 240,
    maxInlineGrowth: 64,
  },
  {
    name: "lean-no-inlining",
    poolStrings: false,
    properties: true,
    optimization: { inlining: false, inline_closure_factories: false },
  },
  {
    name: "lean-minimal-transforms",
    poolStrings: false,
    properties: true,
    poolNumericLiterals: false,
    localNameReserve: 0,
    stableLocalNames: false,
    optimization: {
      inlining: false,
      inline_closure_factories: false,
      identical_function_folding: false,
      function_subsumption: false,
    },
  },
].filter(
  (variant) => requestedVariants.size === 0 || requestedVariants.has(variant.name),
);

function config(variant) {
  const optimization = {
    preset: "maximum",
    ...(variant.optimization ?? {}),
  };
  const optimizationLines = Object.entries(optimization)
    .map(([key, value]) => `${key} = ${JSON.stringify(value)}`)
    .join("\n");
  return `[optimization]
${optimizationLines}

[javascript]
priority = ${JSON.stringify(variant.priority ?? "size-first")}
optimization_level = 15
${variant.optimizations == null ? "" : `optimizations = ${JSON.stringify(variant.optimizations)}`}
cost_model = "brotli"
candidate_search = "off"
pool_numeric_literals = ${variant.poolNumericLiterals ?? true}
local_name_reserve = ${variant.localNameReserve ?? 16}
stable_local_names = ${variant.stableLocalNames ?? true}
function_spelling = ${JSON.stringify(variant.functionSpelling ?? "arrow")}
public_aggregate_abi = ${JSON.stringify(variant.publicAggregateAbi ?? "named")}
${variant.inlineInstructionLimit == null ? "" : `inline_instruction_limit = ${variant.inlineInstructionLimit}`}
${variant.inlineControlFlowLimit == null ? "" : `inline_control_flow_limit = ${variant.inlineControlFlowLimit}`}
${variant.maxInlineGrowth == null ? "" : `max_inline_growth = ${variant.maxInlineGrowth}`}

[mangle]
identifiers = ${variant.identifiers ?? true}
properties = ${variant.properties ?? false}
exports = ${variant.exports ?? false}
pool_strings = ${variant.poolStrings ?? true}
`;
}

requireCanonicalCodecRuntime("jQuery configuration audit measurement");

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
}

const rows = [];
for (const variant of variants) {
  const configPath = join(outputRoot, `${variant.name}.toml`);
  const rawPath = join(portRoot, `jquery-audit-${variant.name}.raw.js`);
  const bundlePath = join(outputRoot, `${variant.name}.js`);
  const minPath = join(outputRoot, `${variant.name}.min.js`);
  writeFileSync(configPath, config(variant));
  const started = performance.now();
  run(compiler, [
    join(portRoot, "entry.lil"),
    "--config",
    configPath,
    "--mode",
    "development",
    "--target",
    "js-module",
    "-o",
    rawPath,
  ]);
  await esbuild({
    absWorkingDir: portRoot,
    entryPoints: [rawPath],
    outfile: bundlePath,
    bundle: true,
    format: "esm",
    platform: "neutral",
    write: true,
  });
  await esbuild({
    absWorkingDir: portRoot,
    entryPoints: [rawPath],
    outfile: minPath,
    bundle: true,
    format: "esm",
    platform: "neutral",
    minify: true,
    write: true,
  });
  const bundle = readFileSync(bundlePath);
  const terser = Buffer.from(
    (
      await terserMinify(bundle.toString("utf8"), {
        module: true,
        compress: { passes: 3 },
        mangle: true,
      })
    ).code,
  );
  const terserPath = join(outputRoot, `${variant.name}.terser.js`);
  writeFileSync(terserPath, terser);
  const row = {
    variant: variant.name,
    milliseconds: performance.now() - started,
    bundle: canonicalCodecSizesForFile(bundlePath, `${variant.name} bundle`),
    esbuild: canonicalCodecSizesForFile(minPath, `${variant.name} esbuild`),
    terser: canonicalCodecSizesForFile(terserPath, `${variant.name} terser`),
  };
  rows.push(row);
  console.log(JSON.stringify(row));
}

writeFileSync(join(outputRoot, "results.json"), `${JSON.stringify({
  schemaVersion: 2,
  codecs: canonicalCodecProvenance("jQuery configuration audit report"),
  rows,
}, null, 2)}\n`);
console.log("configuration/pass-isolation audit: all variants use the Brotli cost model; raw and gzip are diagnostic cross-metrics");
