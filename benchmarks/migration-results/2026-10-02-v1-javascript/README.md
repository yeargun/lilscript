# V1 JavaScript qualification — active

V1 follows N2 at `87d13789`. The sole active milestone remains V1; this is
the working evidence directory, not a completion claim.

The first implementation batch closes the known formation/runtime discoveries:

1. Non-finite semantic constants form unshadowable arithmetic expressions.
2. Typed `Object.values` carries packed record elements into a fresh array;
   element reads retain value-copy semantics. Closed generic transport accepts
   array callbacks and function values held in checked private containers.
3. Complete callable-input proofs follow immutable private alias chains and
   retain every alias dependency. Exports, writes and opaque uses still refuse
   the proof. Indirect reference calls use the existing reference ABI.
4. Generator parameter regions execute in an eager call frame, preserving
   effects, throws, captured parameters, activation identity and arity. The
   iterator body remains lazy. The first development run exposed a missing
   statement-cursor advance in this new lowering; it was repaired without
   increasing resource limits.
5. Direct concrete foreign calls encode/decode products at the existing call
   boundary, using named fields without allocating a callable adapter. A new
   fixture checks argument snapshots, nested getter order, mutation and shared
   reference-valued fields.
6. Four retired positional-ABI fixtures now use named fields while retaining
   their inherited-setter and field-default observations. Existing explicitly
   reflected anonymous-name fixtures pass unchanged. The Marked integration
   fixture binds its discarded nullish expression using accepted syntax.
7. Public nullable fields use their declared absence codec. A null value in a
   nullable field must not bypass the configured null/undefined boundary.
8. Rendered artifacts retain and compare their exact formation contract. A
   semantically reusable program does not make an IIFE artifact eligible as a
   bare-script artifact.
9. Constructor-flow analysis releases temporary scratch charges when its
   analysis ends, including work-refusal paths. Fixed-unit verification avoids
   charging unrelated module-origin searches when an edit cannot change origins.
10. Externally observed functions preserve the first-default parameter's arity
    even when optional default-transport optimization cannot apply. This also
    closes a previously ignored runtime test.

These are semantic formation corrections, not optional tactics. They apply at
effort 0 as well as optimized efforts; no TOML switch can permit wrong behavior.
The initial policy algorithm identity is 86. The subsequent private-frame
correction is algorithm 87: closed applications already exclude host reflection
over their frames, so product helpers must not acquire strict execution merely
for that protection. Module/library guarantees remain in force. This also fixes
failed writes to frozen host objects in classic applications. All 49 affected
boundary/frame/inlining/delivery tests pass, including that independent Node
observation and the corresponding strict-module throw.

Algorithm 88 also reuses the existing lexical initialization proof for
single-use expression helpers over local arguments. It refuses mutable argument
interference, captures, ambiguous placements and reads before initialization.
The lazily built call-site index releases its scratch ownership before edits.
All 36 affected inlining/product/frame/target tests pass. This supplies a general
optimization opportunity without another search tier or a higher effort budget.

`qualify-semantics.py` runs the complete unchanged N2 product/array/reference/
callable/static-data/generator sources against independent ECMAScript oracles
and their existing native goldens, plus a new activation/default fixture. It
selects raw, gzip and Brotli independently at efforts 0 and 13. The case runner
additionally exercises the affected fixtures as application IIFE, ESM, CJS and
bare script.

The remaining ambiguous-product-union/opaque-generic erasure fixture is an
explicit unsupported ABI refusal under the S4 scope decision. Its ledger entry
is retained and is not counted as executable parity. Native exclusions for the
migrated/new host fixtures remain individual source-digest-bound declarations.

## Compiler and independent semantic qualification

[Compiler identity](compiler-identity.json) pins the executable and production
source hashes. [Semantic qualification](semantic-qualification.json) records all
seven complete fixtures at efforts 0 and 13 for each objective: 42 compilations,
42 runtime comparisons and seven independent oracle runs pass.

[Rust qualification](rust-qualification.json) records one broad library run
(2,027 passed, 118 failed, four ignored), then focused repairs and rechecks.
Fifty-four initial failures needed the configured Clang executable. The other
64 were investigated as compiler defects or stale assertions; every initial
failure has a passing recheck. All 17 CLI tests are covered by the initial run
and its one affected recheck. The final arity repair passes all 84 source-legality
tests. This is deliberately not described as a repeated full-suite run on the
final binary.

The pinned external-esbuild consumer test was explicitly run and passed. Two
historical ignored Brotli interaction examples remain historical: their byte
landscapes stopped exhibiting the original valley, and the active finite-oracle
and scheduling tests cover that requirement. An ignore is not an executable
parity pass.

[Language qualification](cases-qualification.json) combines the full 405-case,
24-lane matrix with the 41-case affected recheck. It records 9,594 executable
passes, 102 format masks and 24 explicitly unsupported ABI refusals, with no
unexpected failure remaining. The initial run and rechecks remain separate,
with their own compiler hashes. The latter also correct constructor arity's
hidden receiver, constructor lexical parentage, and CJS single-file delivery
of retained unused definitions when optional DCE is off.

## Matched size qualification

[Candidate 4](compiler-identity-4.json) is the final compiler pin for this stage.
Its [independent semantic recheck](semantic-qualification-4.json) passes all 91
commands. The [full ratchet](ratchet-3.json) passes all 1,926 runtime lanes for
the unchanged 642 programs. Separate raw/gzip/Brotli configurations retain the
same effort, host assumptions, canonical codecs and private bare-script delivery
as the accepted S1 baseline. This measures the intervening migration as a whole,
not just the last two V1 optimizations.

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| 624 language cases | −527 | −369 | −400 |
| 7 applications | −24 | −20 | −19 |
| 11 algorithms | −355 | −168 | −183 |
| Total | **−906** | **−557** | **−602** |

[Size qualification](size-qualification.json) retains every local growth with
the exact old/new emitted code and hashes. Raw improves on 58 programs and grows
on six; gzip improves on 65 and grows on five; Brotli improves on 67 and grows on
17. The remainder ties. Largest local growth is 17 raw, 17 gzip and 12 Brotli
bytes. The ordinary per-corpus aggregate gate accepts the update independently
for every metric; the previous baseline is retained here. No blended score,
changed input or relaxed oracle is used.

Fresh [algorithm competitors](algorithm-competitors.json) reproduce every
eligible pinned Closure winner's bytes and hash. Those competitor runs use
candidate 3; candidate 4's runtime and size results come from the full ratchet
against the identical bars. Fresh [application builds](app-competitors.json)
use candidate 4, reproduce the pinned Closure sizes and pass the JS/native/C
oracles. Both comparison drivers now explicitly request bare private application
output instead of inheriting the new IIFE default. The standalone C application
link also supplies `-lm`, as required by the native floating-point runtime; its
initial seven link failures are recorded separately.

Across the 18 applications/algorithms, LilScript is smaller than Closure under
each objective. The 11 algorithms alone are 43 raw bytes larger, 74 gzip bytes
smaller and 42 Brotli bytes smaller. Individual losses remain. The receipt
classifies every one: helper/value placement, numeric/control-flow forms,
dictionary/branch representation, compressed naming, and the host assumption
boundary for string case conversion. These are accepted residuals under the
owner's aggregate rule, not universal-win claims. Existing inlining,
scalar-replacement, constant-folding and data/naming controls continue to own
the relevant producers.

Still to record: maintained/held-out ports, Vue production profiles, Motion's
delivery gap, and matched compile-cost evidence. V2 follows their completion.

## Library qualification in progress

The first production fleet attempt uses candidate 4. Cnlil's suite passes 56,353
upstream cases, 300,000 fuzz cases and 60,005 join cases, with no cache or
idempotence mismatch. The HTML library compilation was deliberately interrupted
before a result; it is neither a failed test nor a passing qualification.
[Partial receipt](ports-production-1-interrupted.json).

Inspection found path-dependent traversal in the newly used expression call
index and in its existing region-parent helper. The next repair bounds both by
arena nodes/edges, propagates shared placement conservatively, and charges the
parent scan's expression work. A synthetic shared-DAG qualification spans 1,
8, 32, 128 and 512 levels under a linear work allowance. Algorithm identity 89
invalidates cost/replay identities for this accounting change. [All 192 affected target/contract checks](target-qualification.json) pass.
Candidate 5 repeats all 91 semantic commands successfully, and its 54 application/
algorithm artifacts have exactly the candidate-4 hashes. Library qualification
resumes on candidate 5 without repeating cnlil. No fleet timing improvement is
claimed from the interrupted run.

The resumed run retains each completed port under `ports-initial/`; it stopped
after seven completed ports to repair the defects described below. Its
[partial receipt](ports-production-2-interrupted.json) retains all seven results
and the remaining 18 ports. Hast-util-to-html passes all 456 tests. jQuery passes
six behavioral checks and exposes a stale syntax assertion: its default export
uses an export specifier. The repaired check compares the imported default,
`jQuery` and `$` identities and passes on the unchanged compiled artifact.
Together these qualify all seven jQuery checks without rebuilding the package.

Motion's generic array-identity helper now declares its JavaScript boundary
explicitly; the ten-entry checker and all nine isolated upstream comparisons
(three tests under each objective) pass. Its complete production package and
delivery qualification remain pending. MobX builds all four profiles, but its old
test command immediately replaces `dist` with development-only output. Three
production imports consequently fail; 766 tests pass and 11 are upstream skips.
Its corrected test command consumes the completed package. The affected
production checks still need a fresh artifact, because the initial test command
deleted it. [Repair dispositions](port-repairs.json) keep these distinctions.

KaTeX's clean build could not resolve extensions importing the generated core
before atomic installation. The source/config repair maps an explicit bare
provider to the existing core outputs. All five extensions check with no `dist`
directory; its full package still needs production qualification. Its browser
guard now refreshes the site from existing compiler artifacts and fails if they
are missing, avoiding a stale site or an implicit duplicate compilation.

Marked passes 28 of 29 initial tests, including its 660 specification cases and
concatenation check. Its last test expected an obsolete banner and particular
export spelling. The replacement verifies the exact compiler-manifest artifact
hash; it passes against the unchanged output. Public namespace identity and
behavior assertions remain in place. These runs jointly qualify all 29 tests.

Two actual compiler defects are repaired in [candidate 6](compiler-identity-6.json):

- Effort handoff could discard a smaller completed winner when optional work
  ran out during requalification. On the unchanged spelling fixture, a raw
  winner of 493 bytes fell back to 495; Brotli winners of 278 or 274 fell back
  to 292. [Initial evidence](effort-handoff/before.json) retains the ceilings
  and completed checkpoints. Algorithm 90 funds this bounded metadata work
  before exploration, without skipping any admission checks or replenishing
  the compilation's quota. [All nine CLI pairs](effort-handoff/qualification.json)
  pass their runtime goldens and retain every smaller completed checkpoint.
  Raw improves by two bytes in each pair; gzip by 4, 12 and 18; Brotli by
  14, 18 and 18. This is a defect reproducer, not a new calibration corpus.
- Mdast-util-from-markdown's fresh candidate-5 package passes 439 checks and
  fails 307, including stale site checks. Its opener/closer factories lose
  callback effects even in development mode. The reduced language fixture
  `captured_parameter_call_sets.lil` exposes an incomplete propagation worklist:
  when parameter input facts change, capturing closures must be revisited as
  well as the owning function. The repair adds those dependency edges using
  the existing bounded reader index. The tests keep the independent expected
  callback order. [The old compiler](captured-callback/before.json) fails all
  12 production lanes; [the repair](captured-callback/after.json) passes all
  24 JavaScript lanes, including formation controls. Six native lanes explicitly
  exclude this JavaScript-host fixture; the portable native closure check passes.
  A [fresh development library](captured-callback/mdast-development.json) matches
  upstream exactly on all 652 CommonMark inputs. Full package qualification is
  running on candidate 6, followed by Motion, MobX, KaTeX and the remaining ports.

[Focused Rust qualification](compiler-repairs-90/qualification.json) retains
each initial failure and its recheck. The final affected rule/search batch has
124 initial passes and three passing rechecks after correcting the test's JSON
host call and Clang environment. The earlier handoff/admission batches and
their two affected rechecks are also covered. No whole-library rerun is claimed.

The Markdown build was deliberately paused for 705 seconds during the focused
Rust build; its receipt's wall time is not controlled compile-cost evidence.

The runner now preserves build receipts before tests can overwrite them and
reports changed/deleted artifacts afterwards. It skips dependent suites after a
failed build, records the wrapper-to-compiler identity, and discovers named TOML
profiles as well as `lilscript*.toml` when overriding objectives/checks. All 23
runner tests pass. Source-owned `test:build` commands keep clean-build convenience
and release checks; plain `npm test` avoids a duplicate compilation in the fleet.

An older HTML-library receipt used 200 million logical work units; the current
CLI resolves 40 billion with the same TOML. Its historical short compile and
this full package build are not matched timing samples. V1's remaining cost
study must pin equal effective ceilings, in addition to source, effort and
objective. No timing improvement or regression is inferred from those two
different build boundaries.
