import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalCodecProvenance,
  canonicalCodecSizesForFile,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../..");
const build = join(directory, "build");
const resultsPath = join(directory, "results.json");
const webResultsPath = join(root, "web/src/paired-results.json");
const closureVersion = "v20260804";
const costModel = "brotli";
const objectiveConfig = join(root, "comparison/cases/configs/brotli.toml");
const closureSha256 =
  "9cad14d0337b2aaaf9ba8b24446cc45dc737bc34fe591e00835eff700a795d5b";
const compiler = join(root, "target/release/lilscript");
const codecScorer = join(
  root,
  `target/release/lilscript-codec${process.platform === "win32" ? ".exe" : ""}`,
);
const cargo =
  process.env.CARGO ?? join(process.env.HOME ?? "", ".cargo/bin/cargo");
const cc = process.env.CC ?? "clang";
const checkOnly = process.argv.includes("--check");
const outputFlag = process.argv.indexOf("--output");
const explicitOutput =
  outputFlag === -1 ? null : (process.argv[outputFlag + 1] ?? null);
if (outputFlag !== -1 && !explicitOutput) {
  throw new Error("--output requires a report path");
}

const sha256File = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

function command(executable, args, options = {}) {
  return execFileSync(executable, args, {
    cwd: root,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    ...options,
  });
}

function expression(node, target) {
  if (Object.hasOwn(node, "int")) return String(node.int);
  if (Object.hasOwn(node, "bool")) return String(node.bool);
  if (Object.hasOwn(node, "ref")) return node.ref;
  if (Object.hasOwn(node, "call")) {
    const [name, ...args] = node.call;
    return `${name}(${args.map((argument) => expression(argument, target)).join(",")})`;
  }
  if (Object.hasOwn(node, "conditional")) {
    const [condition, truthy, falsy] = node.conditional;
    return `(${expression(condition, target)}?${expression(truthy, target)}:${expression(falsy, target)})`;
  }
  if (Object.hasOwn(node, "binary")) {
    const [operator, left, right] = node.binary;
    const lhs = expression(left, target);
    const rhs = expression(right, target);
    if (target === "lil" || ["==", "<", ">", "<=", ">="].includes(operator)) {
      return `(${lhs}${operator}${rhs})`;
    }
    if (operator === "*") return `((${lhs}*${rhs})|0)`;
    return `((${lhs}${operator}${rhs})|0)`;
  }
  throw new Error(`Unknown expression ${JSON.stringify(node)}`);
}

function statements(items, target, indent = "") {
  const lines = [];
  for (const statement of items) {
    if (Object.hasOwn(statement, "let")) {
      const [name, value] = statement.let;
      lines.push(
        `${indent}${target === "lil" ? "int" : "let"} ${name}=${expression(value, target)};`,
      );
    } else if (Object.hasOwn(statement, "assign")) {
      const [name, value] = statement.assign;
      lines.push(`${indent}${name}=${expression(value, target)};`);
    } else if (Object.hasOwn(statement, "print")) {
      const output = target === "lil" ? "print" : "console.log";
      lines.push(`${indent}${output}(${expression(statement.print, target)});`);
    } else if (Object.hasOwn(statement, "repeat")) {
      const { index, count, body } = statement.repeat;
      const step = target === "lil" ? `${index}++` : `${index}=(${index}+1)|0`;
      const declaration = target === "lil" ? "int" : "let";
      lines.push(
        `${indent}for(${declaration} ${index}=0;${index}<${expression(count, target)};${step}){`,
      );
      lines.push(...statements(body, target, `${indent}  `));
      lines.push(`${indent}}`);
    } else {
      throw new Error(`Unknown statement ${JSON.stringify(statement)}`);
    }
  }
  return lines;
}

function generate(spec, target) {
  const lines = [
    target === "lil"
      ? `// Generated from specs.json schema ${spec.schemaVersion}; do not edit.`
      : `// Generated from specs.json schema ${spec.schemaVersion}; do not edit.`,
  ];
  for (const fn of spec.case.functions) {
    const params =
      target === "lil"
        ? fn.params.map((name) => `int ${name}`).join(",")
        : fn.params.join(",");
    const prefix = target === "lil" ? "pure int" : "function";
    const separator = target === "lil" ? " " : " ";
    lines.push(
      `${prefix}${separator}${fn.name}(${params}){return ${expression(fn.return, target)};}`,
    );
  }
  lines.push(...statements(spec.case.program, target));
  return `${lines.join("\n")}\n`;
}

function sizes(path) {
  return canonicalCodecSizesForFile(path, "paired compiler artifact");
}

function output(executable, args = []) {
  return command(executable, args, { capture: true }).trimEnd();
}

mkdirSync(build, { recursive: true });
if (!existsSync(compiler) || !existsSync(codecScorer)) {
  command(cargo, [
    "build",
    "--release",
    "--bin",
    "lilscript",
    "--bin",
    "lilscript-codec",
  ]);
}
requireCanonicalCodecRuntime("paired compiler gate");
const closureJar = output(join(root, "comparison/install-closure.sh"), [
  closureVersion,
  closureSha256,
]);
const data = JSON.parse(readFileSync(join(directory, "specs.json"), "utf8"));
const results = [];

for (const item of data.cases) {
  const caseDirectory = join(build, item.id);
  mkdirSync(caseDirectory, { recursive: true });
  const lilSource = join(caseDirectory, "main.lil");
  const jsSource = join(caseDirectory, "main.js");
  const lilBase = join(caseDirectory, "lilscript");
  const closureOutput = join(caseDirectory, "closure.js");
  writeFileSync(
    lilSource,
    generate({ schemaVersion: data.schemaVersion, case: item }, "lil"),
  );
  writeFileSync(
    jsSource,
    generate({ schemaVersion: data.schemaVersion, case: item }, "js"),
  );

  command(compiler, [
    lilSource,
    "--target",
    "all",
    "--config",
    objectiveConfig,
    "-o",
    lilBase,
  ]);
  command("java", [
    "-jar",
    closureJar,
    "--js",
    jsSource,
    "--js_output_file",
    closureOutput,
    "--compilation_level",
    "ADVANCED",
    "--language_in",
    "ECMASCRIPT_2021",
    "--language_out",
    "ECMASCRIPT_2021",
    "--warning_level",
    "QUIET",
    "--emit_use_strict=false",
  ]);
  const expected = output("node", [closureOutput]);
  const artifacts = [
    ["LilScript JavaScript", "node", [`${lilBase}.js`]],
    ["LilScript native", lilBase, []],
  ];
  command(cc, ["-std=c11", "-O3", `${lilBase}.c`, "-o", `${lilBase}-from-c`]);
  artifacts.push(["LilScript emitted C", `${lilBase}-from-c`, []]);
  for (const [label, executable, args] of artifacts) {
    const actual = output(executable, args);
    if (actual !== expected) {
      throw new Error(
        `${item.id}: ${label} output differs from generated JavaScript/Closure`,
      );
    }
  }

  const lilscript = sizes(`${lilBase}.js`);
  const closure = sizes(closureOutput);
  if (lilscript[costModel] > closure[costModel]) {
    throw new Error(
      `${item.id}: LilScript ${costModel} ${lilscript[costModel]} exceeds Closure ${closure[costModel]}`,
    );
  }
  results.push({ id: item.id, contract: expected, lilscript, closure });
}

const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  closureVersion,
  compiler: {
    path: "target/release/lilscript",
    version: output(compiler, ["--version"]),
    sha256: sha256File(compiler),
  },
  objectiveConfig: {
    path: "comparison/cases/configs/brotli.toml",
    sha256: sha256File(objectiveConfig),
  },
  costModel,
  objectiveContract: {
    config: objectiveConfig.slice(root.length + 1),
    gateMetric: "brotli",
    diagnosticMetrics: ["raw", "gzip"],
    note: "one Brotli-objective artifact is gated only on Brotli-11; raw and gzip may lose",
  },
  codecs: canonicalCodecProvenance(),
  source: "benchmarks/paired/specs.json",
  results,
};
if (!checkOnly) {
  writeFileSync(resultsPath, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(webResultsPath, `${JSON.stringify(report, null, 2)}\n`);
}
if (explicitOutput) {
  writeFileSync(
    resolve(root, explicitOutput),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}
console.log(
  `Paired benchmark gate passed for ${results.length} generated workloads under the ${costModel} objective.`,
);
