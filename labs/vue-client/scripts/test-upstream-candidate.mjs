import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  checkPinnedUpstream,
  createRunPaths,
  loadSuite,
  parseSuiteArgument,
  resultErrors,
  root,
  summarizeVitest,
  upstreamRoot,
  writeRunReport,
} from "./test-upstream-reference.mjs";

function readJson(path) {
  try {
    return { value: JSON.parse(readFileSync(path, "utf8")), error: null };
  } catch (error) {
    return { value: null, error: error.message };
  }
}

export function auditErrors(audit, suite) {
  const errors = [];
  if (!audit) return ["candidate source audit was not produced"];
  if (audit.completed !== true) errors.push("candidate source audit did not close");
  if (audit.suite !== suite.name) {
    errors.push(`candidate source audit covered ${audit.suite}; expected ${suite.name}`);
  }
  if ((audit.blockedTargetedModules ?? []).length > 0) {
    errors.push("a targeted upstream implementation module reached a load hook");
  }
  if (audit.noTargetedUpstreamImplementationImports !== true) {
    errors.push("candidate source audit did not prove target implementation independence");
  }
  const targets = new Map((audit.targets ?? []).map((target) => [target.package, target]));
  for (const packageName of suite.targetPackages ?? []) {
    const target = targets.get(packageName);
    if (!target) errors.push(`candidate source audit is missing ${packageName}`);
    else {
      if (target.redirects < 1) errors.push(`${packageName} had no candidate redirects`);
      if (target.candidateLoaded !== true) {
        errors.push(`${packageName} candidate facade was not loaded`);
      }
    }
  }
  return errors;
}

export function runCandidate(suiteName = "shared") {
  const paths = createRunPaths(`vuelil-upstream-candidate-${suiteName}`);
  let map;
  let suite;
  let preflightErrors = [];
  try {
    ({ map, suite } = loadSuite(suiteName));
    preflightErrors = checkPinnedUpstream(map, suite, { candidate: true });
  } catch (error) {
    preflightErrors = [error.message];
    map = { upstream: {} };
    suite = { name: suiteName, targetPackages: [], testFiles: [] };
  }

  const args = [
    resolve(root, "node_modules/vitest/vitest.mjs"),
    "run",
    "--config",
    resolve(root, "tooling/vue-test-aliases.mjs"),
    ...(suite.testFiles ?? []).map((file) => file.path),
    "--maxWorkers=1",
    "--no-file-parallelism",
    "--no-cache",
    "--reporter=json",
    `--outputFile=${paths.raw}`,
  ];
  const processResult =
    preflightErrors.length === 0
      ? spawnSync(process.execPath, args, {
          cwd: upstreamRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            VUELIL_TEST_SUITE: suite.name,
            VUELIL_AUDIT_PATH: paths.audit,
          },
          maxBuffer: 64 * 1024 * 1024,
        })
      : { status: null, stdout: "", stderr: "", error: null };
  const raw = existsSync(paths.raw) ? readJson(paths.raw) : { value: null, error: null };
  const audit = existsSync(paths.audit)
    ? readJson(paths.audit)
    : { value: null, error: null };
  const summary = summarizeVitest(raw.value, suite);
  const errors = [
    ...preflightErrors,
    ...(preflightErrors.length === 0
      ? [
          ...resultErrors(raw.value, suite, summary, processResult),
          ...auditErrors(audit.value, suite),
        ]
      : []),
  ];
  if (preflightErrors.length === 0 && !raw.value && raw.error) errors.push(raw.error);
  if (preflightErrors.length === 0 && !audit.value && audit.error) errors.push(audit.error);
  const report = {
    schemaVersion: 1,
    kind: "vuelil-upstream-candidate-test-run",
    scope: suite.name,
    claim: "Scoped candidate test-run result only; this is not VueLil completion evidence.",
    upstream: map.upstream,
    testSources: "unchanged files in the pinned upstream checkout",
    targetPackages: suite.targetPackages ?? [],
    files: summary.files,
    unexpectedFiles: summary.unexpectedFiles,
    totals: summary.totals,
    sourceAudit: audit.value,
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
    outcome = runCandidate(parseSuiteArgument());
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
  if (outcome) {
    console.log(`Candidate report: ${outcome.reportPath}`);
    if (outcome.report.passed) {
      console.log(
        `VueLil passed ${outcome.report.totals.passed}/${outcome.report.totals.tests} unchanged upstream tests in ${outcome.report.totals.files} files with a clean target-source audit.`,
      );
    } else {
      if (outcome.processResult.stdout) process.stdout.write(outcome.processResult.stdout);
      if (outcome.processResult.stderr) process.stderr.write(outcome.processResult.stderr);
      for (const error of outcome.report.errors) console.error(`- ${error}`);
      process.exitCode = 1;
    }
  }
}
