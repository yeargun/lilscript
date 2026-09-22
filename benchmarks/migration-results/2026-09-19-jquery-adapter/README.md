# jQuery Original Boundary Discovery

The single authorized existing-dist discovery passed all eight original
identities: seven tests and one suite container, with no failure or skip. No
compiler, library build, npm command, package installation, or retry ran. The
[frozen inventory](../../libraries/jquerylil.required-tests.json) retains every
actual identity; only the jQuery maintained-workload row was updated.

The [client](discover.mjs) calls the existing `runNodeTestEvidence` owner for the
exact original `node --test test/compat.test.mjs` selection. There is no declared
type prerequisite and no closed output; neither is invented. The required case
inventory was empty deliberately during discovery, so qualification remains
`unverified` and `sourceBuilt` is false. Freezing the observed identities afterward
does not retroactively turn existing-dist evidence into compiler qualification.

Executed child command:

```text
/home/azureuser/.nvm/versions/node/v24.11.1/bin/node /home/azureuser/lilscript/benchmarks/migration-results/2026-09-19-jquery-adapter/discover.mjs
```

The actual external `runBoundedCommand` process-group supervisor used a 90-second
deadline, one-second kill grace, and 64 MiB output cap. It exited 0 without timeout
or signal in 2.683 seconds; this is a shared-host diagnostic, not a speed claim.
The owner received remaining discovery time as its Node-suite ceiling. Adjacent
[supervisor evidence](supervisor.json), [stdout](supervisor.stdout), and
[stderr](supervisor.stderr) retain the command, result, supervision and caller,
Node, and bounded-helper before/after identities.

## Receipts

- [Discovery receipt](existing-dist/receipt.json):
  `c622329fefc6428424d3b3fb342a29dceb875b8ecd360877040e6eeb96191ddb`.
- [Original case report](existing-dist/node/report.json):
  `84ac3f585b27e7525bd5713fc295b7fe729dd0bc9e2a00b0a9fdfea57b53585e`.
- [External supervisor](supervisor.json):
  `e0926e41297b5e85505c2970aa9b2bc82fd813cafa00adba601bf8060d836c60`.

All 15 retained outputs and eight input pins were rehashed after the run. Before,
after and current snapshots agree: 264 source-only files, 270 including existing
distribution files, and project dependencies with 1,829 entries / 1,585 file paths
/ 57,237,165 bytes. Counts include symlink destinations. The before-manifest copy
is `/tmp/lilscript-jquery-before-20260919/benchmarks/libraries/maintained-workloads.json`;
the only subsequent row change is `jquerylil`.

The same Node test process passively reported both candidate loads:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/jquery.esm.js` | 81,828 | `b43df6f9adc4bf1b69d523b6f7a98ff4b9348b481606a4946e5b9ba0e28de0e0` |
| `dist/jquery.cjs` | 81,882 | `322590bb5a5e29ce3f4e2edfc358b7093732e6a7f93cf1a53a875bd8a5a5ec7f` |

The client snapshots source-only and complete original trees, including current
distribution files, plus the complete installed project dependency tree before
and after. Dependency capture includes nested packages and symlink destinations
with bounded traversal. Node, the shared evidence/reporter/passive-load tools,
`.nvmrc`, original test, package/lock/build/config files are pinned. No npm command
executes, so global npm is not claimed as an executed tool dependency. Inherited
Node injection and compiler/npm overrides are cleared and recorded. This is not
a hermetic operating-system claim.

## Original Scope

The test separately loads upstream `jquery@3.7.1` for expected observations and
the candidate `dist/jquery.esm.js` for behavior. The original surface, utilities,
Deferred, public Tween identity/mutation, compact output, and DOM/event assertions
remain unchanged. The Tween test independently expects the step callback to see
the same public object and its `now = 99` mutation to control rendering; discovery
records that caller contract, not a new observation-policy decision.

Candidate `dist/jquery.cjs` is required directly and checked for version and
export alias identity. Both candidate ESM and CJS need exact passive load-byte
evidence. Upstream jQuery is an explicit oracle, not a candidate substitution.
JSDOM runs the original DOM checks in Node, not a real browser. CJS has no full
duplicate behavior suite, and UMD, declaration validation, installed-package
delivery, `check:names`, `check:pack`, and `check:site` remain outside this original
test selection.

Full upstream jQuery suites and the repository's other existing jQuery consumer
suites still require reconciliation and inventories. These eight maintained
identities are not a full jQuery compatibility claim; the machine-readable
inventory retains that outstanding requirement explicitly.

## Source-Build Gap

The original build only compiles when `--compile` is provided, using one compiler
invocation. Its normal path packages an existing raw file, appends the ESM default
export, rewrites named exports for CJS, and produces UMD/declaration outputs. This
does not fit the current shared baseline owner's two-invocation, type-prerequisite
contract, and discovery does not alter that owner.

The original tuned `lilscript.toml`, including historical
`name_ordering = "idiom-converged"`, is preserved unchanged. Current compiler
compatibility and a correctly qualified fresh build remain separate unresolved
work; an existing-dist pass cannot establish either. This README is postrun
narration, not a frozen discovery input.

Read-only source inspection also identifies a schema blocker in the preserved
19:48 release parent, not only the current worktree: its before/after manifests
and current `src/config.rs` share SHA
`157bc865c87df7c637177c34c20ceedfadfb96110536c4cf69b47c998bd735da`.
`JavaScriptConfig` denies unknown fields (line 1472), has no `name_ordering` field
or alias, and the original build's explicit config reaches direct
`toml::from_str` (line 2566). The unused app-config boundary is not a substitute.
Historical `name_ordering` and `idiom_directed_naming` coexisted, so no equivalent
translation has been established. This is source-inspection evidence, not an
executed compiler rejection or a decision about acceptable compilation budgets.

## Failed Consumer Supplement

[consumer-supplement.mjs](consumer-supplement.mjs), SHA
`4e533d675a4376da7632fd2f64eb72d94efc87bbdf9b0009f5b410b83c796b77`,
executed once against the retained ESM through the unchanged repository
`benchmarks/popular/verify-jquery.mjs` artifact-path and expected-SHA override.
The original consumer exited 1 without timeout or signal. Actual external
90-second process-group supervision, one-second kill grace and 64 MiB output
cap were applied; the outer process also exited 1 in 6.459 seconds. This elapsed
time is diagnostic only. There was no retry.

The candidate threw `TypeError: Cannot read properties of undefined (reading
'2')` in `Deferred.pipe` at candidate line 2, column 4144. The unchanged caller
is `deferredPipe` at verifier line 282, evaluated by the assertion at line 294.
The candidate is evaluated before that assertion's upstream operand, so this is
an observed candidate exception, not a recorded unequal pair of returned values.
None of the six completion markers printed. The first two markers occur after
the Deferred checks; this does not mean the preceding core assertions were never
entered. The later data, queue, attributes/CSS and event sections were not reached.

- [Failed supplement receipt](consumer-supplement/receipt.json):
  `c55bddd96a4460c79fc0b81c5bae08168d505a95ec7cf3ad49c43faf46bcb9e3`.
- [External supervisor](consumer-supervisor.json):
  `93837bca72559c418b4004f7cf6c6664b6b8bdbac91eceb113ec206dff1fcac2`.
- [Original stderr](consumer-supplement/consumer.stderr),
  [stdout](consumer-supplement/consumer.stdout), and
  [read-only evidence audit](consumer-audit.json) are retained.

The original script's override skipped compilation and esbuild; its assertions
compare core, Deferred, data, queue, attributes/CSS and events with the popular
lab's installed upstream jQuery. The client required exit 0, all six original
completion markers, and exact passive Node load evidence. These are whole-script
observations, not fabricated Node case identities or replacements for the eight
frozen compatibility identities. The script created an empty lab `build`
directory through its original unconditional `mkdir`; no build files appeared.

All 34 retained outputs and 11 input pins were rehashed; before/after/current
identities agree for 264 source files, 270 full-original files, the maintained
dependency tree (1,829 entries / 1,585 file paths / 57,237,165 bytes), and the
separate popular lab dependency tree (10,602 entries / 8,929 file paths /
357,998,490 bytes). All 15 frozen discovery outputs and its receipt remain
unchanged. Four passive loads in the same child PID bind the original verifier,
selection helper, upstream oracle and candidate ESM to their exact bytes. The
candidate remains SHA `b43df6f9adc4bf1b69d523b6f7a98ff4b9348b481606a4946e5b9ba0e28de0e0`.

The evidence retains the exact candidate, original verifier/helper,
lab package/lock, Node/shared tools, all frozen discovery outputs, and complete
before/after source and installed dependency trees for both the maintained port
and popular lab. No npm command is used. This supplement grants no source-build,
codec, CJS, UMD, installed-package or real-browser credit and does not resolve the
source configuration gap or the other existing consumer suites.

### Read-Only Binding Diagnosis

The retained ESM's `Deferred` is `De=e=>{...}`. Its `pipe:function(){...}` has no
local capture of `arguments`: the nested callback reads `e[i[4]]` and later
assigns `e=null`, resolving `e` to the enclosing `Deferred` parameter. The first
tuple requests slot 2; a zero-argument `Deferred()` leaves that enclosing value
undefined, matching the observed exception. These are lexical observations of
the [exact retained bytes](consumer-supplement/jquery.esm.mjs), not a modified
artifact or another execution.

The pinned maintained source `jquerylil/src/deferred.lil:78-83` instead saves
`pipeFns = fnsArgs`, captures that local in the nested callbacks, and clears it
at line 117; line 256 wraps the method with `JS.methodRest`. This matches the
upstream source's pipe-local `var fns = arguments`. The popular port's
`deferred.lil` is byte-identical to the maintained file (SHA
`2a8ae3a3ae7b24a211633e152306024421e93e18cbea0b4e53dec23daf8fdd43`).
Thus the missing pipe-local binding is not a missing initialization in the
inspected port source. However, these existing distribution bytes have no
qualified source-build/compiler receipt: the responsible historical pass and
whether the full current-source compilation reproduces the defect remain
unproven. The subsequent [generic captured-argument reduction](../2026-09-19-artifact-service/README.md#rest-capture-reduction)
passes on the current compiler, including dynamic indexing, nested mutation,
two invocations, discarded/retained readers and configured search Off/Production.
Only test code changes. It does not clear this original failure, establish its
historical cause or qualify a current source-built jQuery artifact.
