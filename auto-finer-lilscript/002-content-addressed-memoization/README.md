# 002 — Content-addressed memoization of the whole-artifact primitives

**Status: LANDED. Confirmed win, zero size cost.**

## Hypothesis

[001](../001-where-does-compile-time-go/README.md) showed the two whole-artifact primitives dominate
compile time. Both are **pure functions of their input bytes**:

- `compressed_size(bytes, model)` — canonical gzip-9 / Brotli-11 encode (`src/compiler.rs`)
- `analyze_generated_javascript(source)` — lex + 8 validators + region parse + syntax metrics
  (`src/js_peephole/mod.rs`)

Neither was cached. The search revisits identical byte strings constantly: independent structural
plans normalize to the same emission, terminal families re-measure an incumbent they already scored,
and a rejected neighborhood step restores bytes measured one step earlier. In particular
`CodecBudget::measure_reserved` calls `analyze_generated_javascript` on **every** measurement,
including on bytes it validated moments before.

**Prediction:** keying both on a SHA-256 content digest removes a large fraction of the calls while
producing byte-identical output, because a hit returns exactly the value the primitive would have
recomputed. Digesting 26 KB costs ~20 µs against a ~49 ms Brotli encode — a 2500x ratio.

## Implementation

New `src/artifact_memo.rs`:
- `content_digest(bytes) -> [u8; 32]` (SHA-256; `sha2` was already a dependency)
- `ArtifactMemo<K, V>` — `OnceLock<Mutex<HashMap<K, V>>>` with a hard `CAPACITY` of 32768 entries
  and clear-on-overflow. Eviction cannot change an answer, so the cheapest bounded policy is right.
  The encode itself happens **outside** the lock; only the lookup and the insert take it.
- Two statics: `COMPRESSED_SIZE` keyed by `(digest, cost_model_discriminant)` and
  `GENERATED_ANALYSIS` keyed by `digest`.
- `LILSCRIPT_NO_MEMO=1` disables both, purely so the effect can be A/B'd.

Only successful analyses are memoized: a rejection is fatal, and its diagnostic borrows the source
it was built from. `CompressionCostModel::Raw` short-circuits before hashing — its answer is already
the length. The cost-model discriminant is written out by hand rather than derived, so a future cost
model cannot silently alias an existing one's entries.

## Measurement — acorn port, level 15, `candidate_search = "production"`

Same binary, one environment variable apart. Work counters are deterministic and therefore the
primary metric; wall clock on this host is contended by unrelated processes and is reported only for
scale.

| metric | memo off | memo on | change |
|---|---:|---:|---:|
| codec calls | 307 | 279 | **−9.1%** |
| codec MB encoded | 7.86 | 7.15 | **−9.0%** |
| analyze calls | 1620 | 192 | **−88.1%** |
| analyze MB scanned | 41.40 | 4.90 | **−88.2%** |
| output bytes | — | — | **byte-identical** |

Weighting each primitive by its measured cost on a quiet pinned run (Brotli-11 ≈ 1898 ms/MB,
generated-JS analysis ≈ 200 ms/MB) gives whole-artifact primitive work of **23.2 s-equivalent → 14.6
s-equivalent, a 37% reduction**.

## Findings

1. **The analysis tax was almost entirely redundant.** 88% of `analyze_generated_javascript` calls
   were re-validating bytes already validated. That validation exists to guarantee no unparseable
   artifact is ever scored; running it once per distinct byte string preserves the guarantee exactly.
2. **The codec was already near-unique.** Only 9% of encodes were repeats. This is the honest
   negative half of the result: memoization does **not** solve the Brotli problem. The search
   genuinely proposes distinct byte strings, so the fix there must attack the 49 ms *per-encode
   constant* or the *number of distinct proposals*, not repetition. That is [005](../005-*/).
3. **Byte-identical output** across the A/B confirms the memo is behavior-preserving, as the purity
   argument required.

## End-to-end result

Measured after 004's lexer cache and 005's decline memo also landed, as one combined A/B
(`LILSCRIPT_NO_MEMO=1` reproduces the pre-change behavior exactly). jQuery port, level 13, three
interleaved rounds, CPU time (user+sys):

| round | caches on | caches off |
|---|---:|---:|
| 1 | 198.7 s | 273.8 s |
| 2 | 389.9 s | 373.5 s |
| 3 | 260.2 s | 477.3 s |
| **minimum** | **198.7 s** | **273.8 s** |
| median | 260.2 s | 373.5 s |

**−27% CPU on the minimum, −30% on the median, with a byte-identical artifact** (all six runs
produced the same SHA-256). Round 2 inverts, which is what a shared host looks like; the minimum is
the defensible statistic because contention can only add time.

## Verdict

Kept. Small win on the codec, large win on the analysis, no size cost, no output change.
