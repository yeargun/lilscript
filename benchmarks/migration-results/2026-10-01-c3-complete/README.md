# C3 — calibration infrastructure complete

The [pre-registered acceptance](../../../docs/migration/c3-acceptance.md) is
complete at `28cd60f9`. The production compiler behavior and search schedule are
unchanged: all Rust production diffs from the C2 qualification pin are comments;
the only executable Rust change is the NO3 architecture test. Measurements use
the immutable C2 compiler and codec binaries:

- compiler SHA-256 `cbfaa03a736fa3e91d5445b0a3cafdd86290bdd62c478c93daf0584efc281056`;
- codec SHA-256 `51365f6ee712fa8040dbc54ad4d74e82c08b311ae5204455d154341396b892ac`;
- runner SHA-256 `3e245395d3bea4485b531abf19f29d9e5a1166e3f086425f66259cde874b266c`;
- frozen corpus manifest SHA-256 `d45cf8843838a3f9158b32b413e70fcde0e6269239170251f4e57cce7d6bb845`.

[Training](training.json) contains 24 complete objective cells: eight generic
programs in four families, each 16–164 KB of source, measured independently for
raw, gzip and Brotli at effort 13. Every cell runs pruning `on`, diagnostic
`audit`, and `off`; every emitted artifact passes an independent input-dependent
oracle and an independent canonical codec measurement. Explanations retain
resolved policy, transitive source hashes, exact artifacts, every terminal
trial, resource work, stops, current CPU samples and tool identities. Auditing
always produced the same artifact as ordinary `on` mode.

## Training result

| Objective | On bytes | Off bytes | Delta, off − on | On → off judgments | On → off optional work | Stops per mode |
|---|---:|---:|---:|---:|---:|---:|
| Raw | 364,386 | 364,386 | 0 | 28 → 190 | 4,938,526,039 → 7,449,622,722 | 4 |
| Gzip 9 | 55,665 | 55,558 | −107 | 34 → 283 | 7,486,149,764 → 9,770,014,544 | 4 |
| Brotli 11 | 39,146 | 39,128 | −18 | 107 → 311 | 7,425,712,620 → 9,888,418,304 | 4 |

All **209 audited rejections** have nonnegative immediate exact deltas. The
gzip wins (−106 on `strings-384`, −1 on `records-384`) and Brotli wins (−7 on
`strings-128`, −11 on `strings-384`) arise only after following a rejected
starting path into later refinements. This establishes why an audit score alone
cannot prove final search safety. Raw spends 51% more optional work for no byte
gain with pruning off; gzip spends 31% more and Brotli 33% more.

Leave-one-family-out selection uses only the other three training families.
Raw retains `on` in every fold. Gzip selects `off` in every fold, but its
savings are concentrated in the string family plus one byte in records.
Brotli's fold holding out strings selects `on` and consequently predicts none
of the observed 18-byte gain. The evidence therefore does **not** support a
default change. Q3 owns a broader calibrated search policy; explicit
`proxy_pruning = "off"` remains available now for users who accept the cost.

The current on-mode CPU baseline has a median of 3.202 seconds per objective
cell, a 0.308–123.152 second range and 260.742 seconds summed across per-cell
medians. These are retained warm samples, not reconstructed A1 history and not
a cross-version speed claim. Four large cells per objective reach an existing
search/store limit, which is explicit in the receipts; pruning off is not a
global exhaustive maximum.

## Protected evaluation

[Evaluation](evaluation.json) contains nine complete cells from three disjoint
algorithm families: CRC table lookup, linear-space edit distance and interval
search. Parameter selection consumes only the completed training receipt, and
the runner refuses a different compiler, codec, policy, Node binary, corpus or
training hash. All artifacts pass independent references (Python's native CRC,
a full edit-distance matrix and a linear interval scan).

| Objective | On bytes | Off bytes | Delta | On → off judgments | On → off optional work |
|---|---:|---:|---:|---:|---:|
| Raw | 9,268 | 9,268 | 0 | 36 → 107 | 20,245,841 → 24,891,940 |
| Gzip 9 | 4,753 | 4,753 | 0 | 10 → 138 | 34,534,798 → 64,723,756 |
| Brotli 11 | 3,529 | 3,529 | 0 | 135 → 135 | 35,290,872 → 35,290,872 |

No protected workload changes final bytes. Raw/gzip pruning saves work; Brotli
does not prune these smaller algorithm artifacts. There are no stops. Current
on-mode CPU medians range from 0.014 to 0.651 seconds (1.193 seconds summed).
This validates the protocol and rejects a training-only default promotion; it
does not claim that three authored algorithms represent the external library
fleet. Maintained and external boundaries remain V1 work.

## Architecture, labels and repairs

`tests/no3-allowlist.json` is empty. The focused source gate passes both tests
and now requires it to remain empty. Every former library citation is retained
in [historical-no3.json](../../calibration/historical-no3.json), while
[policy-provenance.md](../../calibration/policy-provenance.md) states the generic
legality, exact choice or bounded compatibility owner for each old rule and
numeric bound.

`exact-bytes-v1` labels every eligible negative delta as smaller, including one
byte. Robustness, runtime parity and compilation cost are separate evidence;
historical thresholds and verdicts are never rewritten. The public benchmark
contract now carries this versioned policy.

Two input-authoring failures are retained. The first training fixture used a
JavaScript conditional expression unsupported by LilScript; twelve already
complete cells were fully reverified rather than rerun. The first protected CRC
fixture spelled arrays as `Array<int>` instead of `int[]` and failed before
artifact generation. Both repairs kept expected values and policy fixed and
preceded any result for the affected valid program. Receipts:
[training syntax failure](training-source-syntax-failure.json),
[pre-evaluation training](training-before-evaluation-type-repair.json), and
[evaluation type failure](evaluation-source-type-failure.json).

Verification: five Python protocol tests, five Node size/contract tests and the
two focused NO3 Rust tests pass. The working artifacts and per-invocation files
remain under `~/lilscript-work/measurements/2026-09-30-c3-complete/`; the report
hashes are preserved by the committed copies here.
