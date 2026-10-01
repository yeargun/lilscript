# Q4 const data, static schemas and reconstruction

Q4 completes its [acceptance contract](../../../docs/migration/q4-acceptance.md).
Algorithm 65 / search 33. `identity.json` pins the release source files, compiler
and codec. The compiler SHA-256 is
`7523692475f4879dc956c85d80599b4d643e20494f325cf41b14cb89e5a2306b`.
The final public-product diagnostic was added after this release pin; its separate
focused debug test passes (`boundary.log`). It only refuses the newly identified
unsupported boundary; the release measurements below predate that guard.

## Correctness and production bytes

`qualification.json` and `focused.log`: 36 affected tests pass. Required bounded
const/define evaluation, readonly aliases/escapes, exact graph construction,
static schema forwarding, lazy permissions/TDZ and actual helper module owners
are covered. `boundary.log` adds the passing stored-public-product refusal.
Initial failures and follow-up logs remain; the lazy-expression renumbering and
compiler budget/capture fixes are included. No full library or fleet suite ran.

`artifacts.json`: all 30 JS builds pass independent Node observations and canonical
codec remeasurement of every delivered file. Each objective is built independently
at effort 13. Values below are delivered objective bytes (literal / eager / lazy
permission), not three encodings of one selected artifact.

| Input | Raw | gzip9 | Brotli11 |
|---|---|---|---|
| Public frozen dictionary | 1696 / 1696 / 1696 | 542 / 523 / 523 | 360 / 352 / 352 |
| Two dictionaries, preserved modules | 3138 / 2690 / 2690 | 940 / 936 / 936 | 612 / 559 / 559 |
| Static schema, folding off / on | 104 / 71 | 114 / 85 | 92 / 70 |
| Existing mutable data, Q3 / Q4 | 7883 / 7883 | 2673 / 2673 | 2091 / 2091 |

The schema comparison disables inlining and compares folding off/on; it does not
isolate this patch against Q3. Lazy permission brings no additional selected size
win here. Existing exact mutable literals retain their sound eager choices.
Public checks include keys, null prototype, aliases and freezing. Module output
checks include all helper files and dependency ownership.

The shared required lowering also passes GCC and pinned Clang 18 with constant
folding disabled, overridden SIZE=8 and a loop-built const array (64, 9). Generated
C SHA-256: `17d7793ad4c98527c525d030958455f08e08bb53d9f6fd14b66e197cd854a35d`.
This does not claim native static storage or public const ABI; N2 owns both.

## Representation and runtime tradeoffs

`runtime.json` uses a generic immutable 512-entry scalar dictionary, seven forced
forms, five alternating process pairs each and three identical baseline controls.
All forms satisfy the independent oracle. Raw/gzip/Brotli are measured exactly.

| Form | Raw | gzip9 | Brotli11 | Startup ratio | First-use ratio | Hot-access ratio |
|---|---:|---:|---:|---:|---:|---:|
| Literal | 34037 | 1728 | 958 | 1 | 1 | 1 |
| Front-coded | 30655 | 487 | 272 | 1.08 | 0.97 | 0.33 |
| Columns | 33119 | 1372 | 538 | 1.00 | 1.14 | 0.34 |
| Dictionary | 6380 | 1221 | 565 | 0.99 | 0.97 | 0.37 |
| Lazy front-coded | 30691 | 508 | 286 | 0.67 | 17.84 | 0.38 |
| Lazy columns | 33155 | 1403 | 579 | 0.71 | 13.96 | 0.39 |
| Lazy dictionary | 6416 | 1251 | 589 | 0.67 | 12.68 | 0.41 |

Ratios are paired medians; see bootstrap verdicts in the JSON (some startup
changes are inconclusive). Lazy forms reduce unused heap but defer decoding to
first use and add bytes here. Heap deltas after GC are not allocation counts.
This microfixture is not a fleet runtime guarantee. Both startup and recurring
reconstruction permissions are required; lazy remains opt-in at every effort.
The literal incumbent is always available. Full artifact/helper bytes determine
selection, independently for each codec.

Typed target helpers replace the old source-prelude mechanism, with normal
ownership/dependencies/placement and naming. See the acceptance record for finite
evaluator/schema scope, preserved mutable-literal legality, canonical enum
representations and the public stored-product adapter remaining in D2.
