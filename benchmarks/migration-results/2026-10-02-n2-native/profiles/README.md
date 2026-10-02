# N2 physical storage and profiles

Algorithm 85 adds independently configurable native stack storage and closed
generic function storage. Both default on. Unknown/escaping closure lifetimes,
suspended activations, polymorphic calls and nominal layout binders keep their
general owned/tagged representation. Shared throw-free and storage facts remove
exception checks only when no physical conversion can fail. Disabling these
native controls does not change JavaScript policy identity.

The exact commands, source identities, compiler hashes, logs and samples are
retained beside each cohort. Generated C/header hashes replace duplicate runtime
text; executables are not checked in. Individual logs, configurations and
receipts are stored in each cohort's deterministic `artifacts.tar.gz`; summary
JSON and file hashes remain directly readable. Early failed fixtures and the unsuccessful
WASI build remain separate from qualified cohorts.

## Physical workloads

`physical-qualified/qualification.json`: all 34 steps passed on pin
`n2-physical-2`, with independent ECMAScript observations, efforts 0/13,
both physical controls off/on, strict GCC, Clang 18 ASan/UBSan and zero owners
after execution. Closure tests include aliases, managed cells, escaping nested
closures, recursion, loops and catch/finally. Generic tests include independent
binders, polymorphic calls, callbacks and references. Passed ownership,
exception, generator and task rows are retained in the two earlier cohorts;
those cohorts as a whole contain fixture/driver failures and are not passes.

`physical-measurements/measurement.json` records five serial end-to-end process
samples per lane and separate allocation-counter builds. Each run checks an
independent checksum. Inlining is disabled to isolate physical costs; both
workloads run 1,000,001 iterations. Times include process startup, including Node
startup for JavaScript. These are not steady-state or fleet measurements.

| Workload | Native controls/profile | Median CPU ms | Allocations | Executable bytes |
|---|---|---:|---:|---:|
| Closures | General storage / speed | 62.64 | 2,000,002 | 34,776 |
| Closures | Stack only / speed | 20.74 | 0 | 34,696 |
| Closures | Both / speed | 20.40 | 0 | 34,696 |
| Closures | Both / balanced | 24.88 | 0 | 26,664 |
| Closures | Both / size | 729.27 | 0 | 27,240 |
| Generics | General storage / speed | 547.46 | 1,000,002 | 48,440 |
| Generics | Generic only / speed | 60.78 | 1 | 39,808 |
| Generics | Both / speed | 60.72 | 1 | 39,808 |
| Generics | Both / balanced | 62.82 | 1 | 31,776 |
| Generics | Both / size | 509.51 | 1 | 32,360 |

Emitted JavaScript medians were 95.43 ms for closures and 42.05 ms for generics.
Native generic specialization improves substantially over tagged native storage
but remains slower than JavaScript in this process measurement. GCC's `-Os`
profile was slower and slightly larger than `-O2` here. The configured profile
selects a compiler strategy; it does not promise the smallest or fastest artifact
on every input. No measured counter overhead enters the timing executables.

## WASI and library cores

`wasi-qualified/qualification.json`: all 24 source/build/run steps passed on
`n2-physical-3`, covering strict binary64, ownership, Unicode/regex, JSON,
generators, tasks and both new storage fixtures. The profile uses WASI SDK 34,
`wasm32-wasip1`, a 1 MiB stack and Wasmtime 49.0.1. Official download identities and
SHA256 digests are in `wasi-toolchains.json`. The SDK's deprecated
`wasm32-wasi` spelling diagnoses under warnings-as-errors. An unused upstream
`setjmp.h` include is omitted by the checked numeric amalgamation recipe;
no exception-handling extension is required merely to compile that library.

`library-cores/qualification.json` retains independent checksums, emitted
JavaScript, strict GCC speed/size/balanced, Clang ASan/UBSan and real CLI-produced
WASI artifacts. All steps passed on `n2-physical-3`.

The KaTeX 0.16.22 core ports the complete Unicode classification algorithms and
all seven script groups; its independent oracle executes the unchanged MIT
upstream bodies after removing Flow annotations. It is not a KaTeX parser or
renderer. The Marked 18.0.10 core imports the byte-identical complete archived
rules module, exercising 117 constant regexes against 24 strings; its independent
oracle uses the archived upstream pattern table. It is not the full Marked
lexer/parser. Sources, adaptation details and licenses live in `tests/native/cores`.

| Core | Native speed ms | Balanced ms | Size ms | Emitted JS ms |
|---|---:|---:|---:|---:|
| KaTeX Unicode | 90.26 | 76.79 | 310.02 | 55.14 |
| Marked rules | 137.56 | 137.61 | 216.57 | 109.77 |

These are five process samples with startup included. Native is slower on both
cores; no universal native speed advantage or complete-library result is claimed.
Earlier discovery measurements are retained for diagnosis, not combined with
the final flag-controlled samples.

## Portable algorithm graphs

`algorithms/qualification.json` records all 571 passing steps on `n2-corpus-2`.
All eleven unchanged `comparison/algorithms` graphs agree with their independent
JavaScript implementations on each checked-in golden vector and the complete
ordered host-access trace. Runtime providers supply the inputs; no source
algorithm is replaced by a precomputed result. GCC speed/size/balanced,
Clang ASan/UBSan and emitted JavaScript pass. A shared provider expands the first
vector 2,048 times for five alternating process samples; one expanded sanitizer
sample checks correctness. The two parser graphs are included in full.

| Algorithm | Speed CPU ms | Balanced CPU ms | Size CPU ms | JS CPU ms |
|---|---:|---:|---:|---:|
| aggregate-ledger | 0.72 | 0.73 | 1.78 | 39.17 |
| collection-geometry | 1.05 | 1.03 | 2.41 | 40.99 |
| dictionary-template-router | 1.32 | 1.51 | 8.51 | 45.48 |
| helper-sharing | 0.71 | 0.73 | 1.90 | 31.49 |
| large-event-analytics | 1.41 | 2.42 | 10.38 | 63.73 |
| policy-specialization | 0.74 | 0.74 | 2.62 | 37.15 |
| shape-invoice-pipeline | 0.86 | 0.89 | 2.83 | 44.65 |
| state-machine-parser | 1.05 | 2.09 | 6.96 | 43.03 |
| stateful-packet-decoder | 1.06 | 2.10 | 8.24 | 37.29 |
| static-rule-engine | 0.75 | 0.76 | 1.80 | 34.91 |
| string-dictionary | 1.05 | 1.14 | 5.33 | 41.61 |

These short process measurements include startup, input-provider work and
printing. Node startup dominates several rows, so the table does not establish
steady-state native superiority. Binary/source sizes and every raw sample are
retained separately. The KaTeX and Marked core results above still show native
runtime costs worth improving.

## Corpus and consolidated tests

`corpus/corpus-clang-sanitizers.json` records 161 executable cases in both
formation and production modes: all 322 case-lanes pass under strict Clang18,
ASan and UBSan with unrecoverable sanitizer errors. The remaining 242 cases have
explicit source/config/oracle-digest-bound exclusions: 147 need C test providers,
84 invoke JavaScript host facilities, seven observe JavaScript prototypes or
module exports, and four use obsolete positional-ABI controls owned by V1.
Neither exclusions nor successful target checks count as runtime passes.

The initial GCC cohort and the five-case repair cohort cover the same native
execution cases. The `inline for` fixture now preserves its required source pin
under formation-only settings. One dynamically called `JsValue` fixture is
explicitly JavaScript-only even though production optimization happens to
resolve its callee. The seven obsolete native Record failure-ledger entries are
removed after their golden executions pass.

The consolidated Rust selection ran 138 native/configuration/cache/timing tests
once. It found stale negative expectations and callback ABI assertions, plus
real defects in nominal generic-storage binding, virtual rest-argument presence
and legacy empty class-field initialization. The 17-test repair selection
includes the shared product-identity checker test. Sixteen passed; the remaining
fixture counted a newly entered captured TDZ slot as a leak. Its count assertion
now runs before entering that binding's block; the final focused rerun passes and is
retained in `last-closure-test.log.gz`. Compiler failures and their diagnostics remain in the
initial compressed logs (original hashes in `corpus/logs.json`), rather than being overwritten by the passing results.
