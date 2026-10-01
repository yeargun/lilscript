# S3 implementation contract

S3 is complete after S2 (`f15d2ee8`). The owner now requests
the remaining migration in order, with focused verification after substantial
implementation batches. This record defines the implemented boundary and its completion evidence.

## Calls and shared rewrites

- [x] Derive bounded call frequency and callable-identity evidence, preserving
  unknown/recursive/host cases. Consume it in call decisions and report work.
- [x] Normalize supported terminal return trees and resolve known private
  calls without changing receiver, callee-before-arguments, exception, default,
  `arguments`, identity, reflection or initialization semantics.
- [x] Extend inlining across module boundaries and into bodies creating nested
  closures. Preserve fresh captured storage per invocation, lexical scope,
  argument effects, snapshots and supported native ownership. Unsupported
  suspension, dynamic frames and unproved escape retain calls.
- [x] Remove unnecessary field-written reference parameter cells and waiting
  product-argument storage when a complete use/evaluation proof permits it.

## Alternatives and controls

- [x] Extend existing call specialization beyond physical product transport;
  expose useful constant-argument and body-sharing choices through existing
  objective judging. Keep a legal original representation available whenever
  the size of duplication/sharing is uncertain.
- [x] Share identical private implementations only when identity/captures and
  reflection permit it; parameterized sharing must honor its runtime-risk
  permission. Preserve individually optimized raw/gzip/Brotli results.
- [x] Register all producers and invalidation, honor independent `inlining`,
  `call-specialization` and `helper-sharing` vetoes, and document situations,
  defaults and compilation/size/runtime tradeoffs in the TOML reference.
- [x] Remove replaced recognizers with their coverage. Formation-created
  subjects remain with the existing Q1/S4 ownership until their replacement
  actually sees them; do not create another optimizer or bespoke search loop.

## Completion evidence

- [x] One focused batch covering new call shapes, independent runtime oracles,
  alias/TDZ/reentry/throw/capture negatives, module boundaries and tactic vetoes.
  Exercise native for changed shared semantics and relevant sanitizer paths.
- [x] Retain source/compiler/config/output identities and independent raw,
  gzip and Brotli results on affected generic programs. Report actual work and
  candidate growth; reserve broader timing claims for matched measurements.
- [x] Update checklist, coverage, history and handoff together. Commit the
  completed milestone before beginning Q1. Do not resume S1's deferred full
  library/CLI/paired-CPU qualification or repeat whole-library runs per edit.

The completed implementation keeps suspension, dynamic frames, escaping function
identity and unproved captures conservative. Shared source inlining only duplicates
empty identity transports unconditionally; nonempty repeated bodies and literal
specializations are target alternatives judged after naming and printing. The old
six-node target duplication prior is removed. Primitive helper matching is bounded
and uses complete direct-call, capture and initialization evidence; recurring
parameterized sharing requires `helper-sharing = "on"`.

Normalized select branches retain their path ranges, and late expression copies
run the permitted literal fold before measurement. Native captured cells outside
the module entry region use activation storage, and declared inline result cells
participate in ownership cleanup. Relevant GCC/Clang and sanitizer profiles pass.

The [evidence](../../benchmarks/migration-results/2026-10-01-s3-complete/README.md)
contains focused behavior and size comparisons. It is not full-fleet, paired-CPU
or native-completion qualification. Q1 owns finer per-site/joint representation
moves; G1 owns the remaining measured identifier-choice loss. The historical
receiver-alias and R7 adapter requirements retain their proper G3/S4 ownership
in [coverage.md](coverage.md).
