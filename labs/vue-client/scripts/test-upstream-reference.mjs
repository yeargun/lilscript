import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
export const upstreamRoot = resolve(root, "upstream/vue");
export const testMapPath = resolve(root, "compatibility/test-map.json");

function toPosix(path) {
  return path.split(sep).join("/");
}

function run(program, args, cwd) {
  return spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 64 * 1024 * 1024,
  });
}

function git(args) {
  const result = run("git", args, upstreamRoot);
  if (result.error) return { value: "", error: result.error.message };
  if (result.status !== 0) {
    return {
      value: "",
      error: (result.stderr || result.stdout || "git failed").trim(),
    };
  }
  return { value: result.stdout.trim(), error: null };
}

export function loadSuite(name = "shared") {
  const map = JSON.parse(readFileSync(testMapPath, "utf8"));
  if (map.schemaVersion !== 1) {
    throw new Error(`Unsupported Vue test-map schema ${map.schemaVersion}`);
  }
  const suite = map.suites?.[name];
  if (!suite) throw new Error(`Unknown Vue upstream test suite: ${name}`);
  return { map, suite: { name, ...suite } };
}

export function checkPinnedUpstream(map, suite, { candidate = false } = {}) {
  const errors = [];
  if (Number(process.versions.node.split(".", 1)[0]) < 24) {
    errors.push(`Node >=24 is required; found ${process.version}`);
  }
  if (!existsSync(resolve(upstreamRoot, "package.json"))) {
    errors.push("pinned Vue checkout is missing; run npm run setup:upstream");
    return errors;
  }
  const packageDefinition = JSON.parse(
    readFileSync(resolve(upstreamRoot, "package.json"), "utf8"),
  );
  if (packageDefinition.version !== map.upstream.version) {
    errors.push(
      `Vue version is ${packageDefinition.version}; expected ${map.upstream.version}`,
    );
  }
  const revision = git(["rev-parse", "HEAD"]);
  if (revision.error) errors.push(revision.error);
  else if (revision.value !== map.upstream.revision) {
    errors.push(`Vue revision is ${revision.value}; expected ${map.upstream.revision}`);
  }
  const status = git(["status", "--porcelain", "--untracked-files=no"]);
  if (status.error) errors.push(status.error);
  else if (status.value !== "") errors.push("Vue checkout contains tracked changes");
  for (const test of suite.testFiles ?? []) {
    if (!existsSync(resolve(upstreamRoot, test.path))) {
      errors.push(`missing upstream test file ${test.path}`);
    }
  }
  if (candidate) {
    for (const packageName of suite.targetPackages ?? []) {
      const definition = map.packages?.[packageName];
      if (!definition?.candidateFacade) {
        errors.push(`no candidate facade is mapped for ${packageName}`);
      } else if (!existsSync(resolve(root, definition.candidateFacade))) {
        errors.push(`missing candidate facade ${definition.candidateFacade}`);
      }
    }
  }
  return errors;
}

function readJson(path) {
  try {
    return { value: JSON.parse(readFileSync(path, "utf8")), error: null };
  } catch (error) {
    return { value: null, error: error.message };
  }
}

export function summarizeVitest(raw, suite) {
  const byPath = new Map();
  for (const result of raw?.testResults ?? []) {
    const path = toPosix(relative(upstreamRoot, result.name));
    byPath.set(path, result);
  }
  const files = (suite.testFiles ?? []).map((expected) => {
    const result = byPath.get(expected.path);
    byPath.delete(expected.path);
    const tests = (result?.assertionResults ?? []).map((assertion) => ({
      name: assertion.fullName,
      status: assertion.status,
      durationMs: assertion.duration ?? null,
      failureMessages: assertion.failureMessages ?? [],
    }));
    return {
      path: expected.path,
      status: result?.status ?? "not-run",
      expectedTests: expected.tests,
      tests,
      counts: {
        total: tests.length,
        passed: tests.filter((test) => test.status === "passed").length,
        failed: tests.filter((test) => test.status === "failed").length,
        pending: tests.filter((test) => test.status !== "passed" && test.status !== "failed")
          .length,
      },
      durationMs:
        typeof result?.startTime === "number" && typeof result?.endTime === "number"
          ? result.endTime - result.startTime
          : null,
      message: result?.message || null,
    };
  });
  return {
    files,
    unexpectedFiles: [...byPath.keys()].sort(),
    totals: {
      files: files.length,
      tests: files.reduce((total, file) => total + file.counts.total, 0),
      passed: files.reduce((total, file) => total + file.counts.passed, 0),
      failed: files.reduce((total, file) => total + file.counts.failed, 0),
      pending: files.reduce((total, file) => total + file.counts.pending, 0),
    },
  };
}

export function resultErrors(raw, suite, summary, processResult) {
  const errors = [];
  if (processResult.error) errors.push(processResult.error.message);
  if (processResult.status !== 0) {
    errors.push(`Vitest exited with status ${processResult.status ?? "unknown"}`);
  }
  if (!raw) errors.push("Vitest did not produce a readable JSON report");
  if (raw && raw.success !== true) errors.push("Vitest reported an unsuccessful run");
  if (summary.unexpectedFiles.length > 0) {
    errors.push(`Vitest ran unexpected files: ${summary.unexpectedFiles.join(", ")}`);
  }
  for (const file of summary.files) {
    if (file.status !== "passed") errors.push(`${file.path} status is ${file.status}`);
    if (file.counts.total !== file.expectedTests) {
      errors.push(
        `${file.path} ran ${file.counts.total} tests; expected ${file.expectedTests}`,
      );
    }
    if (file.counts.failed !== 0 || file.counts.pending !== 0) {
      errors.push(`${file.path} did not pass every test`);
    }
  }
  const expectedTotal = (suite.testFiles ?? []).reduce(
    (total, file) => total + file.tests,
    0,
  );
  if (summary.totals.tests !== expectedTotal) {
    errors.push(`suite ran ${summary.totals.tests} tests; expected ${expectedTotal}`);
  }
  return errors;
}

export function createRunPaths(prefix) {
  const directory = mkdtempSync(resolve(tmpdir(), `${prefix}-`));
  return {
    directory,
    raw: resolve(directory, "vitest.json"),
    report: resolve(directory, "report.json"),
    audit: resolve(directory, "source-audit.json"),
  };
}

export function writeRunReport(path, report) {
  writeFileSync(path, `${JSON.stringify(report, null, 2)}\n`);
}

export function parseSuiteArgument(argv = process.argv.slice(2)) {
  let name = "shared";
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--suite") {
      if (!argv[index + 1]) throw new Error("--suite requires a value");
      name = argv[index + 1];
      index += 1;
    } else if (argument.startsWith("--suite=")) {
      name = argument.slice("--suite=".length);
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  return name;
}

export function runReference(suiteName = "shared") {
  const paths = createRunPaths(`vuelil-upstream-reference-${suiteName}`);
  let map;
  let suite;
  let preflightErrors = [];
  try {
    ({ map, suite } = loadSuite(suiteName));
    preflightErrors = checkPinnedUpstream(map, suite);
  } catch (error) {
    preflightErrors = [error.message];
    map = { upstream: {} };
    suite = { name: suiteName, testFiles: [] };
  }

  const args = [
    resolve(upstreamRoot, "node_modules/vitest/vitest.mjs"),
    "run",
    "--project=unit",
    ...(suite.testFiles ?? []).map((file) => file.path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--reporter=json",
    `--outputFile=${paths.raw}`,
  ];
  const processResult =
    preflightErrors.length === 0
      ? run(process.execPath, args, upstreamRoot)
      : { status: null, stdout: "", stderr: "", error: null };
  const parsed = existsSync(paths.raw)
    ? readJson(paths.raw)
    : { value: null, error: "report was not created" };
  const summary = summarizeVitest(parsed.value, suite);
  const errors = [
    ...preflightErrors,
    ...(preflightErrors.length === 0
      ? resultErrors(parsed.value, suite, summary, processResult)
      : []),
  ];
  if (!parsed.value && preflightErrors.length === 0) errors.push(parsed.error);
  const report = {
    schemaVersion: 1,
    kind: "vue-upstream-reference-test-run",
    scope: suite.name,
    claim: "Scoped test-run result only; this is not VueLil completion evidence.",
    upstream: map.upstream,
    testSources: "unchanged files in the pinned upstream checkout",
    files: summary.files,
    unexpectedFiles: summary.unexpectedFiles,
    totals: summary.totals,
    passed: errors.length === 0,
    errors,
  };
  writeRunReport(paths.report, report);
  return { report, reportPath: paths.report, processResult };
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) {
  let outcome;
  try {
    outcome = runReference(parseSuiteArgument());
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
  if (outcome) {
    console.log(`Reference report: ${outcome.reportPath}`);
    if (outcome.report.passed) {
      console.log(
        `Vue ${outcome.report.upstream.version} reference passed ${outcome.report.totals.passed}/${outcome.report.totals.tests} tests in ${outcome.report.totals.files} files.`,
      );
    } else {
      if (outcome.processResult.stdout) process.stdout.write(outcome.processResult.stdout);
      if (outcome.processResult.stderr) process.stderr.write(outcome.processResult.stderr);
      for (const error of outcome.report.errors) console.error(`- ${error}`);
      process.exitCode = 1;
    }
  }
}
