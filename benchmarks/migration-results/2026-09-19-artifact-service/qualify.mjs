import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { runBoundedCommand } from "../../../finer/tools/bounded-command.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const profileArguments = process.argv.slice(2).filter(value => value.startsWith("--profile="));
assert(profileArguments.length <= 1, "duplicate profile argument");
const profile = profileArguments[0]?.slice("--profile=".length) ?? "debug";
assert(["debug", "release"].includes(profile), "profile must be debug or release");
const cargoProfile = profile === "release" ? ["--release"] : [];
const semanticCore = process.argv.includes("--semantic-core");
const moduleBoundary = process.argv.includes("--module-boundary");
const publicIntegration = process.argv.includes("--public-integration");
const testFilters = process.argv.slice(2).filter(value => value.startsWith("--test-filter="))
  .map(value => value.slice("--test-filter=".length));
const testsOnly = testFilters.length > 0;
assert(testFilters.every(value => /^[a-zA-Z0-9_:]+$/.test(value)), "test filters must be nonempty Rust test-name fragments");
assert(new Set(testFilters).size === testFilters.length, "duplicate test filter");
assert(process.argv.slice(2).every(value => ["--semantic-core", "--module-boundary", "--public-integration"].includes(value) || value.startsWith("--test-filter=") || value.startsWith("--profile=")), "unknown qualification argument");
assert(!publicIntegration || (!semanticCore && !moduleBoundary), "public-integration is a separate focused cohort");
assert(!testsOnly || (!semanticCore && !moduleBoundary && !publicIntegration), "explicit test filters select tests-only qualification");
const root = resolve(directory, "../../..");
const target = join(root, "target/qualification-internal-provenance-20260918-v3");
const runDirectory = join(directory, `run-${new Date().toISOString().replaceAll(":", "-")}`);
mkdirSync(runDirectory, { recursive: true });
const env = {
  ...process.env,
  PATH: "/home/azureuser/.nvm/versions/node/v24.11.1/bin:/home/azureuser/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
  CARGO_TARGET_DIR: target, CARGO_TERM_COLOR: "never", CARGO_BUILD_JOBS: "2", RUST_BACKTRACE: "0",
  LILSCRIPT_NATIVE_CC: "/usr/bin/cc",
  LILSCRIPT_NATIVE_CLANG: "/tmp/lilscript-native-core-20260913/toolchain/root/usr/bin/clang-18",
  CC: "/usr/bin/cc",
};
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const identity = path => ({ path: relative(root, path), sha256: hash(readFileSync(path)), bytes: statSync(path).size });
const save = (name, value) => writeFileSync(join(runDirectory, name), JSON.stringify(value, null, 2) + "\n");
function inputs() {
  const files = [];
  function walk(path) {
    if (!existsSync(path)) return;
    if (statSync(path).isDirectory()) for (const name of readdirSync(path).sort()) walk(join(path, name));
    else files.push(identity(path));
  }
  for (const path of [".cargo", ".nvmrc", "Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml", "src", "tests", "examples", "vendor", "finer/tools/semantic-integration.py", "finer/tools/bounded-command.mjs"]) walk(join(root, path));
  files.push(identity(fileURLToPath(import.meta.url)));
  files.sort((a, b) => a.path.localeCompare(b.path));
  return { sha256: hash(JSON.stringify(files)), files };
}
const receipt = {
  schema: 1, scope: `focused ${profile} artifact admission, shared recipe ownership, public scoped service and original example consumers; not milestone certification`,
  started: new Date().toISOString(), commands: [],
  profile, buildJobs: env.CARGO_BUILD_JOBS, semanticCore, moduleBoundary, publicIntegration, testsOnly, testFilters,
  limitations: ["Frontend allocation accounting incomplete", "No full fleet or release-speed qualification", "Native byte metrics describe C/header delivery, not linked executables", "Retained descriptors are not an implemented recipe replay engine", "Shared sessions do not yet migrate LSP, playground or Lilpack"],
  buildEnvironment: Object.fromEntries(Object.entries(env).filter(([key]) =>
    /^(?:RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|RUSTC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER)$/.test(key)
    || /^CARGO_PROFILE_/.test(key) || /^CARGO_TARGET_.*_RUSTFLAGS$/.test(key))),
};
if (testsOnly) {
  receipt.scope = `explicit affected ${profile} Rust test filters only; no CLI, delivery, library-fleet or speed qualification`;
  receipt.limitations.push("Only the recorded library test filters execute; prior production evidence is not rerun");
}
async function run(label, command, args, expected = 0) {
  const start = performance.now();
  const result = await runBoundedCommand(command, args, { cwd: root, env, encoding: "utf8", timeoutMs: 600_000, maxBuffer: 64 * 1024 * 1024 });
  writeFileSync(join(runDirectory, `${label}.stdout`), result.stdout ?? "");
  writeFileSync(join(runDirectory, `${label}.stderr`), result.stderr ?? "");
  receipt.commands.push({ label, command, args, status: result.status, signal: result.signal, error: result.error?.message, supervision: result.supervision, ms: performance.now() - start });
  save("receipt.json", receipt);
  const diagnostics = command === "cargo"
    ? (result.stdout ?? "").split("\n").filter(line => line.startsWith("{")).map(line => JSON.parse(line)).filter(row => row.reason === "compiler-message" && row.message.level === "error").map(row => row.message.rendered).join("\n")
    : result.stdout.slice(-8000);
  assert.equal(result.status, expected, `${label}: ${result.stderr}\n${diagnostics}`);
  console.log(`${label}: passed (${Math.round(performance.now() - start)} ms)`);
  return result;
}
function artifacts(result) {
  return result.stdout.split("\n").filter(Boolean).map(line => JSON.parse(line)).filter(row => row.reason === "compiler-artifact" && row.executable);
}

function buildProfiles(rows) {
  if (profile === "release") for (const row of rows) {
    assert.equal(row.profile.opt_level, "3", "release optimization level");
    assert.equal(row.profile.debug_assertions, false, "release debug assertions");
    assert.equal(row.profile.overflow_checks, false, "release overflow checks");
  }
  return rows.map(row => ({ path: relative(root, row.executable), target: row.target.name, profile: row.profile, features: row.features }));
}

function storeOracle(result, label, prefix, count) {
  const rows = kind => result.stderr.split("\n").filter(line => line.startsWith(`${prefix}-${kind} `))
    .map(line => JSON.parse(line.slice(`${prefix}-${kind} `.length)));
  const artifacts = rows("artifact");
  const summaries = rows("summary");
  assert.equal(artifacts.length, count);
  assert.equal(summaries.length, 1);
  for (const row of artifacts) {
    assert.equal(row.sha256, hash(row.javascript));
    assert.equal(row.sizes.raw, Buffer.byteLength(row.javascript));
  }
  save(`${label}.json`, { artifacts, summary: summaries[0] });
  return identity(join(runDirectory, `${label}.json`));
}

async function captureOracle(library, label, filter, prefix, count) {
  const result = await run(label, library, [filter, "--exact", "--nocapture"]);
  assert.match(result.stdout, /test result: ok\. 1 passed; 0 failed;/);
  return storeOracle(result, label, prefix, count);
}

const initial = inputs();
save("inputs-before.json", initial);
receipt.inputSha256 = initial.sha256;
async function qualify() {
  if (profile === "release") assert.equal(Object.values(receipt.buildEnvironment).filter(Boolean).length, 0,
    "release qualification requires no inherited compiler/profile overrides; see buildEnvironment");
  receipt.tools = [
    (await run("node-version", "node", ["--version"])).stdout.trim(),
    (await run("rust-version", "rustc", ["-Vv"])).stdout.trim(),
    (await run("cargo-version", "cargo", ["-V"])).stdout.trim(),
    (await run("cc-version", "/usr/bin/cc", ["--version"])).stdout.trim(),
  ];
  const builtTests = artifacts(await run("build-tests", "cargo", ["test", ...cargoProfile, "--lib", ...(testsOnly ? [] : ["--bin", "lilscript"]), "--no-run", "--message-format=json"]));
  receipt.testBuildProfiles = buildProfiles(builtTests);
  const library = builtTests.find(row => row.target.kind.includes("lib") && row.profile.test).executable;
  const cliTests = builtTests.find(row => row.target.kind.includes("bin") && row.profile.test)?.executable;
  receipt.testBinaries = [identity(library), ...(testsOnly ? [] : [identity(cliTests)])];
  const filters = testsOnly ? testFilters : publicIntegration ? [
    "compiler_service::", "compilation_policy::", "compilation_policy_evidence_tests::",
    "semantic_program::integrated_javascript_tests::", "semantic_program::native_tests::closure_tests::",
  ] : semanticCore ? [
    "compiler_service::", "compilation_policy::", "compilation_policy_evidence_tests::",
    "lexer::", "parser::", "module::", "literal::", "output_budget::", "semantic::", "semantic_program::",
  ] : [
    "compiler_service::", "compilation_policy::", "compilation_policy_evidence_tests::",
    "lexer::", "parser::", "module::", "literal::", "output_budget::", "semantic::",
    "semantic_program::verify::", "semantic_program::module_contract::",
    "semantic_program::artifacts::", "semantic_program::artifact_provenance::",
    "semantic_program::artifact_lifetime_tests::", "semantic_program::search_entries::",
    "semantic_program::publication::search::", "semantic_program::search_tests::",
    "semantic_program::search_staged_tests::", "semantic_program::observation_output_tests::",
    "semantic_program::native_tests::", "semantic_program::native_struct_tests::",
    "semantic_program::implementation_identity::", "semantic_program::recipe_descriptor_tests::",
    "semantic_program::observation_edit_tests::", "semantic_program::publication::edits::",
    "semantic_program::publication::rewrites::",
  ];
  if (moduleBoundary) filters.push(...[
    "attributes_purity_errors_to_the_dependency_module",
    "compiles_and_tree_shakes_a_module_graph",
    "cyclic_imports_observe_live_export_updates",
    "initializes_each_module_once_in_a_static_cycle",
    "links_two_module_cycle_with_exported_value",
    "permits_deferred_cyclic_module_value_reads",
    "rejects_eager_cyclic_module_value_reads",
    "reports_missing_exports_and_links_static_module_cycles",
    "resolves_reexports_through_a_static_cycle",
    "emits_typed_foreign_modules_as_native_esm_imports",
    "preserves_side_effect_foreign_esm_imports",
    "tree_shakes_unused_foreign_imports_through_barrel_reexports",
    "preserves_surviving_dependency_functions_as_esm_chunks",
    "split_rejects_more_mandatory_lazy_chunks_than_max_chunks",
    "splits_only_shared_modules_that_meet_size_policy",
  ].map(name => `compiler::tests::${name}`));
  receipt.focusedTests = [];
  receipt.focusedFailures = [];
  for (const [index, filter] of filters.entries()) {
    try {
      const result = await run(`tests-${index}`, library, [filter, ...(publicIntegration ? ["--nocapture"] : [])]);
      const match = result.stdout.match(/test result: ok\. (\d+) passed; (\d+) failed;/);
      assert(match && Number(match[1]) > 0 && match[2] === "0", `empty or failed filter: ${filter}`);
      receipt.focusedTests.push({ filter, passed: Number(match[1]) });
    } catch (error) {
      receipt.focusedFailures.push({ filter, message: error.message });
      console.error(`failed filter ${filter}; continuing focused checks with the same binary`);
    }
  }
  if (!testsOnly) await run("cli-unit-tests", cliTests, []);
  assert.equal(receipt.focusedFailures.length, 0, `${receipt.focusedFailures.length} focused filters failed; see per-filter logs`);
  if (testsOnly) return;
  if (publicIntegration) {
    const combined = { stderr: readFileSync(join(runDirectory, "tests-3.stderr"), "utf8") };
    receipt.publicCombinedOracle = storeOracle(combined, "public-combined-oracle", "public-combined", 6);
    const rows = readFileSync(join(runDirectory, "tests-4.stderr"), "utf8").split("\n")
      .filter(line => line.startsWith("native-public-factory-artifact "))
      .map(line => JSON.parse(line.slice("native-public-factory-artifact ".length)));
    assert.equal(rows.length, 2);
    for (const row of rows) {
      assert.equal(hash(row.c_source), row.c_sha256);
      assert.equal(hash(row.header_source), row.header_sha256);
      assert.equal(Buffer.byteLength(row.c_source), row.qualification.c_source_bytes);
      assert.equal(Buffer.byteLength(row.header_source), row.qualification.header_source_bytes);
      assert.equal(row.native_report.resources.codec_work, 0);
      assert.equal(row.native_report.ledger_after_finish.retained_bytes, 0);
    }
    save("native-public-factory.json", { artifacts: rows });
    receipt.nativePublicFactory = identity(join(runDirectory, "native-public-factory.json"));
  }
  if (!publicIntegration) {
    receipt.stringGroupOracle = await captureOracle(library, "string-group-oracle",
      "semantic_program::search_string_group_tests::manual_joint_string_pool_has_a_qualified_finite_oracle_beyond_singleton_discovery",
      "string-group", 18);
    receipt.namingNeighborhoodOracle = await captureOracle(library, "naming-neighborhood-oracle",
      "semantic_program::search_naming_neighborhood_tests::source_name_overrides_have_a_qualified_finite_structural_neighborhood",
      "naming-neighborhood", 24);
  }
  const built = artifacts(await run("build-cli", "cargo", ["build", ...cargoProfile, "--bin", "lilscript", "--bin", "lilscript-codec", "--example", "semantic-integrated", "--message-format=json"]));
  receipt.deliveryBuildProfiles = buildProfiles(built);
  const compiler = built.find(row => row.target.name === "lilscript").executable;
  const scorer = built.find(row => row.target.name === "lilscript-codec").executable;
  const driver = built.find(row => row.target.name === "semantic-integrated").executable;
  receipt.binaries = [identity(compiler), identity(scorer), identity(driver)];
  receipt.exampleConsumers = [];
  for (const mode of ["direct", "search", "paired", "native"]) {
    const output = join(runDirectory, `example-${mode}`);
    await run(`example-${mode}`, "python3", [
      join(root, "finer/tools/semantic-integration.py"),
      "--driver", driver, "--codec", scorer,
      "--fixture", join(root, "src/semantic_program/fixtures/integrated-architecture"),
      "--output", output, "--mode", mode, "--pairs", "1", "--edits", "4",
      ...(mode === "paired" ? ["--retain-at", "1"] : []),
    ]);
    const summary = JSON.parse(readFileSync(join(output, "summary.json")));
    assert.equal(summary.qualified, true);
    assert.equal(summary.issues.length, 0);
    receipt.exampleConsumers.push({ mode, summary: identity(join(output, "summary.json")), contract: identity(join(output, "contract.json")), runs: summary.runs.length });
  }
  const config = join(runDirectory, "lilscript.toml");
  writeFileSync(config, "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n");
  const source = join(runDirectory, "entry.lil");
  writeFileSync(source, "export int byte(int value){return(value&255)+1;}\n");
  const args = [source, "--config", config, "--backend", "semantic", "--target", "js-module", "--explain", "json"];
  const stdout = await run("cli-stdout", compiler, args);
  const output = join(runDirectory, "entry.mjs");
  writeFileSync(output, stdout.stdout);
  const report = JSON.parse(stdout.stderr);
  const score = report.artifacts[report.winners[2]];
  assert.equal(hash(stdout.stdout), score.sha256);
  assert.equal(Buffer.byteLength(stdout.stdout), score.raw);
  const fileOutput = join(runDirectory, "file.mjs");
  await run("cli-file", compiler, [...args, "--output", fileOutput]);
  assert.equal(readFileSync(fileOutput, "utf8"), stdout.stdout);
  const measured = JSON.parse((await run("cli-rescore", scorer, ["--json", output])).stdout).artifacts[0];
  assert.equal(measured.raw, score.raw);
  assert.equal(measured.brotli11, score.brotli11);
  await run("cli-parse", "node", ["--check", output]);
  const observations = await run("cli-execute", "node", ["--input-type=module", "-e", `const m=await import(${JSON.stringify(pathToFileURL(output).href)});console.log(JSON.stringify([m.byte.name,m.byte.length,m.byte(-1),m.byte({valueOf(){return 511}})]));`]);
  assert.equal(observations.stdout, '["byte",1,256,256]\n');
  const development = await run("cli-development", compiler, [...args, "--mode", "development"]);
  const developmentReport = JSON.parse(development.stderr);
  assert.equal(developmentReport.search.proposals, 0);
  assert.equal(hash(development.stdout), developmentReport.artifacts[developmentReport.winners[2]].sha256);
  const legacyFamilyConfig = join(runDirectory, "legacy-family.toml");
  writeFileSync(legacyFamilyConfig, "[optimization]\nfor_of_specialize_family=1\n");
  const legacyFamily = await run("cli-legacy-family-unsupported", compiler, [source, "--config", legacyFamilyConfig, "--backend", "semantic", "--target", "js-module"], 1);
  assert.equal(legacyFamily.stdout, "");
  assert.match(legacyFamily.stderr, /does not support optimization\.for_of_specialize_family/);
  const invalid = join(runDirectory, "unsupported.lil");
  writeFileSync(invalid, "export int answer(int value=17){return value;}\n");
  const refused = await run("cli-unsupported", compiler, [invalid, "--config", config, "--backend", "semantic", "--target", "js-module"], 1);
  assert.equal(refused.stdout, "");
  assert.match(refused.stderr, /direct module checking does not yet support source parameter defaults/);
  const portable = join(runDirectory, "portable.lil");
  writeFileSync(portable, "int twice(int value){return value*2;}print(twice(21));\n");
  const native = join(runDirectory, "portable");
  const all = await run("cli-all", compiler, [portable, "--config", config, "--backend", "semantic", "--target", "all", "--output", native, "--explain", "json"]);
  const allReport = JSON.parse(all.stderr);
  assert.equal(hash(readFileSync(`${native}.c`)), allReport.native_sha256);
  assert.equal(hash(readFileSync(`${native}.js`)), allReport.artifacts[allReport.winners[2]].sha256);
  assert.equal((await run("cli-native-execute", native, [])).stdout, "42\n");
  assert.equal((await run("cli-all-js-execute", "node", [`${native}.js`])).stdout, "42\n");
  receipt.delivered = { source: identity(source), output: identity(output), canonicalSizes: measured, report };
}

try {
  await qualify();
  receipt.passed = true;
} catch (error) {
  receipt.passed = false;
  receipt.failure = error.stack;
  process.exitCode = 1;
  console.error(error.message);
} finally {
  const final = inputs();
  save("inputs-after.json", final);
  receipt.inputsStable = initial.sha256 === final.sha256;
  if (!receipt.inputsStable) { receipt.passed = false; process.exitCode = 1; }
  receipt.completed = new Date().toISOString();
  save("receipt.json", receipt);
  console.log(JSON.stringify({ directory: runDirectory, passed: receipt.passed, inputsStable: receipt.inputsStable }));
}
