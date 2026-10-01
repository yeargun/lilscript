# S4 charCodeAt number contract

1. Add `[language] char_code_at = "legacy" | "number"`, independent of target,
   effort and optimization permissions. Keep the compatibility default until
   qualified port adoption in D3/V2. The migration lint retains its meaning-
   preserving rewrite for legacy source and does not rewrite the new contract.
2. Resolve the selected operation once in the checker. A distinct primitive
   identity retains the return type and out-of-range behavior through shared
   folding, JavaScript generation, native generation and verification.
3. Under `number`, preserve UTF-16 code units in range and NaN out of range on
   both targets. Explicit `| 0` preserves the old numeric result; `codeUnitAt`
   retains its existing in-range precondition and bounds checks.

Prediction: number-valued calls can omit unnecessary int32 normalization. The
meaning-preserving migration spelling may be the same size as legacy output.
Check dynamic and folded calls, UTF-16/lone surrogates, native NaN, config/type
admission and the legacy lint after the coherent implementation. Compare a
small matched migration case separately from new-semantics cases, by objective;
no full library/fleet rerun or CPU claim.

## Implemented and checked

The number contract is a checked primitive identity, so later passes never
recover its meaning from a method spelling or an effort setting. The shared
evaluator returns the UTF-16 code unit or NaN. JavaScript emits the direct
method; native uses a double-valued helper with NaN out of range. The legacy
primitive and `codeUnitAt` retain their distinct contracts.

Five focused checks pass: runtime/folded JavaScript results under every
objective and both check modes; native results at levels 0/13 with folding
disabled and enabled; the existing independent Node method oracle, extended
with number-valued reads; and legacy/new-contract lint behavior. Surrogate
pairs, a lone surrogate, negative/end indices, empty strings and explicit
integer conversion are covered. CLI/lint/LSP release builds pass. No full
library/fleet suite was rerun.

Compiler pin: `/home/azureuser/lilscript-work/bin/s4-char-code-1/lilscript`, SHA-256
`a4dcf50c09d0c80a747d37091f4e3e00fd4c042541a7d317e182187f4c7fe4f1`.
Algorithm 29 / search 31 / walk 10 / local facts 12. Source and binary identities
are retained in [identity.json](identity.json); [measure.mjs](measure.mjs) writes
the independent Node oracles, TOML settings and [comparison](comparison.json).
Every delivered objective size equals the compiler's exact score.

| Scope | Raw before → after | Gzip 9 before → after | Brotli 11 before → after |
|---|---:|---:|---:|
| Unchanged-contract control | 145 → 145 | 126 → 126 | 107 → 107 |
| Opt-in bounded read, identical behavior | 74 → 72 | 92 → 90 | 65 → 64 |

The private-product control retains the legacy contract. The bounded-read
comparison opts into `number`, but the function checks nonempty input before
reading its first code unit, making the complete behavior identical under both
contracts. The number/NaN case is a new capability without an old comparator.
The contract remains opt-in until D3/V2's qualified port/default transition.
No general runtime, fleet, native-completion or Closure claim is made.
