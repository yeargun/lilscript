# S4 contracts implementation batch

S4 remains active. This batch implements statement/type-parameter identities,
canonical checked expression types, declaration-owned defaults, shared primitive
and target binding facts, early native capability diagnostics, checked array
reads, value `with`, generic methods, and typed receivers/rest parameters.
The default expressions run in the callee and may capture earlier parameters.
JavaScript preserves public `length`; native transports the supplied count.
Spread snapshots precede later argument effects. Source optimization vetoes
remain in force; there are no new semantic assumptions or effort gates.

`raw_domains.rs`, `quiet.rs`, `root_constants.rs` and the literal-only default
preparation were replaced with shared fact consumers and declaration lowering.
This is not a claim that the full transitional-pass audit is finished.

The first release pin is
`/home/azureuser/lilscript-work/bin/s4-contracts-1/lilscript`, SHA-256
`74deaaf981783420ce335b17d1896568b724b2ffce3b2bcd9225608bf0ed9346`.
[identity.json](identity.json) records its source inputs. Algorithm 20, search
31 and walk 10 identify the contracts batch; shared local facts are version 9.

Thirty-two S4-focused checks passed after the implementation batch. An additional
host-alias regression passes after its failure-driven fix. Existing admission,
type relation, call/parameter contract, method/rest and default checks also pass:
the default filter alone passed 56 with one ignored. No full suite was run.
The old forged-signature fixture was corrected because default *values* no longer
distinguish otherwise identical function types; required versus optional arity
still does. A synthetic parameter's signature and ordinal remain verified.

[comparison.json](comparison.json) retains 15 objective cells against immutable
G3 artifacts. The five selected development cases total unchanged **1279 raw,
738 gzip-9, 591 Brotli-11 bytes**, each total using its own objective's output.
The frozen C3 `records-128` program passes its eight independent numeric inputs
under all three objectives. G3 refused that program, so this is a correctness
result, not a byte-saving comparison. No fleet, Closure or timing claim.

The carried Object.hasOwn fixture now declares its host binding explicitly.
Initial production qualification exposed a call-contract verifier error after
forwarding an extern alias; formation alone passed. Both callee-owned defaults
and host omission preserve the actual argument count. The verifier now retains
that checked contract when forwarding changes the visible callee declaration,
while continuing to reject synthesized arguments or caller materialization.
The initial failing report is retained; the follow-up release/report records
the correction separately.

The final batch pin is `s4-contracts-2`, SHA-256
`36458f95ec83d67f85c2cfdca1703dc716fa6af07b8067acfd6dd77f7edc64e2`.
[followup-identity.json](followup-identity.json) records its source inputs.
The [follow-up](object-has-own-followup.json) passes all eight requested lanes:
formation raw plus optimized raw/gzip/Brotli, each as script and module. The
stale expected-failure entry is removed. The separate optimized alias/arity
oracle and forged-call-contract check pass. The unchanged-size and frozen-record
measurements above remain attributed to pin 1; they were not rerun for this
verifier-only correction.

Remaining S4 work includes shapes/variants/ABI enums, boundary normalization,
the definite-assignment port migration, remaining indexing contracts, observed
generic classes/struct transport, the host catalog and the full shared-consumer
audit. Y2/Y3/Y4 remain unapproved semantic amendments. Q2 has not started.
