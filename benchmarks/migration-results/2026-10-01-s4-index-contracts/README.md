# S4 indexed-access contracts

1. Split native `codeUnitAt` from legacy `charCodeAt`: the former checks its
   precondition and traps; the latter retains the compatibility result pending
   port migration. Audit binary typed-array reads and make every element kind
   trap out of range. Preserve their specified write/conversion semantics.
2. Extend the same guarded `.get(i)` lowering to binary typed arrays, using the
   operation catalog's element/length type. Both operands evaluate once before
   the guard; the out-of-range branch does not load storage.
3. Qualify a coherent batch with focused shared JavaScript/native cases and
   negative boundary executions. No broad library rerun. No optional flag for
   required semantics; existing `javascript.checks` controls development checks.

Prediction: shared guarded reads avoid duplicate target implementations; native
in-range operations keep constant-time checks. JavaScript output for previously
supported programs should be unchanged. New capability comparisons are behavior
checks, not compression wins against a compiler that refused the source.

## Result

Implemented native UTF-16 string views, code-unit traps and bounds traps for all
nine binary-array kinds, plus shared nullable binary `.get`. A pin-22 probe
found that development binary reads with unused results lost their check before
formation. A semantic Program flag now tells the shared effects owner which
reads can trap. Candidate publication preserves it, including when optional
rules are off. Production JavaScript keeps its previous unchecked contract.

Six focused checks pass across the coherent implementation and failure-driven
followups. Positive binary/string behavior matches Node under GCC/Clang O0/O2,
UBSan and Clang ASan (seven native profiles); all nine binary kinds and
`codeUnitAt` trap at negative/end/extreme indices. Public optimized native and
all three JavaScript objective routes preserve unused-result checks through
helper calls. Test fixture mapping/header/configuration mistakes and the prior
compiler's missed check are retained. No full suite was rerun.

The [field batch](../2026-10-01-s4-field-contracts/README.md) owns the shared
release pin, complete source identity and small objective comparison. Its
`checked-indexing` release oracle also covers all four dead-result read kinds
and binary nullable reads. Legacy `charCodeAt` migration remains open; this
batch does not change its compatibility contract or claim native completion.
