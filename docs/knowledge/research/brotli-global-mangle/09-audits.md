# In-tree jQuery compiler audits

Parent: [index](README.md). These are **already-emitted** LilScript
artifacts from `benchmarks/popular/audit-jquery-configs.mjs`, not
post-hoc token rewrites. Each row is a different compiler knob.
Scored here with the same Node Brotli diagnostic as the mutations.

`jquery-lil-raw` (102681 / 33283) is the current public raw emit,
included as the reference line. It is not one of the audit files.

| Id | What the knob does | Raw | gzip-9 | br5 | br11 |
|---|---|---:|---:|---:|---:|
| audit-positional | `properties=true`, public aggregates positional | 96401 | 37202 | 36116 | **33165** |
| jquery-lil-raw | current public raw (not an audit) | 102681 | 38948 | 36306 | 33283 |
| audit-lean | pool off, properties on | 99476 | 37234 | 36197 | 33373 |
| audit-no-string-pool | `pool_strings=false` | 99476 | 37234 | 36197 | 33373 |
| audit-balanced | `priority=balanced` | 97869 | 37623 | 36559 | 33569 |
| audit-mangled-exports | `exports=true` | 98338 | 37639 | 36539 | 33582 |
| audit-no-inlining | inlining off | 97680 | 37638 | 36584 | 33613 |
| audit-no-number-pool | numeric pool off | 98660 | 37685 | 36595 | 33614 |
| audit-function-spelling | `function` instead of arrow | 103062 | 37815 | 36706 | 33647 |
| audit-no-reserve | `local_name_reserve=0` | 98379 | 37656 | 36558 | **33648** |
| audit-readable | identifiers off, pool off | 229262 | 71176 | 66166 | 58144 |

More audit files live in [extra.json](extra.json) `moreAudits`.
Selected extras, same scorer:

| Id | Raw | br11 | Note |
|---|---:|---:|---|
| audit-positional | 96401 | **33165** | best full-port Brotli in the set |
| audit-lean-balanced | 98509 | **33216** | beats lean and public raw |
| jquery-lil-raw | 102681 | 33283 | public raw reference |
| audit-lean-no-inlining | 98773 | 33285 | outlined, still beats lean |
| audit-lean-minimal-transforms | 99069 | 33357 | |
| audit-lean | 99476 | 33373 | |
| audit-lean-inline-24 / 48 / 96 | 99584–103651 | 33380–33484 | more inline, worse Brotli |
| cluster @ 98379 / 33648 | 98379 | 33648 | current, no-reserve, unstable-locals, internal-properties, no-scalar, no-subsumption — **identical bytes** |
| audit-comma-conditionals | 115537 | 35555 | extra syntax lost |
| audit-performance | 103528 | 34080 | priority ≠ size |
| audit-lean-debug-names | 197597 | 52895 | identifiers off |
| audit-readable | 229262 | 58144 | identifiers off |
| audit-slim | 72920 | 24635 | **smaller program**, not a fair row |

`audit-current` matching `audit-no-reserve` means the checked-in
“current” raw was emitted with reserve effectively off, or the
two configs produced the same spelling. Either way, 33648 vs
lean 33373 is the reserve-shaped hole again.

`audit-lean-balanced` (33216) is **157 Brotli under lean** at
**967 less raw**. Balanced is not automatically larger transfer.
`audit-balanced` without lean (33569) *is* larger. The knob
combination matters; the priority name does not.

Inlining **up** (24 → 96) grew Brotli on the lean-inline series.
Inlining **off** (lean-no-inlining 33285) beat lean (33373).
The inliner is not monotone in transfer. Keep both ends in the
IR beam, which the compiler already does.

## Local optimum: smaller raw, worse Brotli

`audit-lean` is **3205 raw bytes smaller** than `jquery-lil-raw` and
**90 Brotli bytes larger**. `audit-balanced` is 4807 raw smaller than
lean-adjacent public raw and **286 Brotli larger** than positional.
A raw-first or “looks minified” ranking would keep the wrong emit.

`audit-no-string-pool` is **byte-identical** to `audit-lean`. On this
port the string pool was already empty or disabled in the lean
config (`poolStrings: false` is the lean default). Turning the pool
off is not an ablation if it never fired.

## `local_name_reserve=0` is the compiler’s own uniquify

`audit-no-reserve` vs `audit-lean`: −97 raw, **+275 Brotli**. The
compiler stopped reserving the first N short spellings for lexical
locals. Similar functions stopped sharing `a`, `b`, `c`. That is the
same gravity as `uniquify-short` in [02](02-reuse.md), at legal
amplitude.

The mutation `uniquify-short` on jquery-lil-raw is +21269 Brotli
because it breaks **every** later use, not just the reserved prefix.
The audit is the shippable version of the same mistake.

## Function spelling

Forcing `function` instead of arrow: +3286 raw, **+274 Brotli** vs
lean. Arrow is already the file’s culture. The ROM word `function`
is cheap as a **keyword you already emit**; flooding extra
`function` tokens by rewriting arrows is not the same as “use the
dictionary.” See [04](04-dictionary-as-names.md).

## Positional aggregates

Best Brotli in this table (33165). Smaller raw (96401) **and**
smaller transfer. This is not a mangling trick: it is a
representation change (named fields → slots) that removes property
names the codec would otherwise have to copy. It only stays legal
when the public ABI allows it. The lesson for mangling: **the
largest legal wins are often “emit fewer distinct names,” not
“pick prettier letters.”**

## Inlining off

`audit-no-inlining` is 796 raw **smaller** than lean and 240 Brotli
**larger**. Outlined helpers look tidy and compress locally; the
inlined form repeats bodies the window can copy. A raw-local
inliner heuristic that only asks “did the function get smaller?”
will keep the outlined emit.

## Readable

58144 Brotli vs 33165 positional: **+25 KB** for long names. That
is the unminified tax, not a codec quirk. Once names are short,
the remaining fight is reuse and alphabet (tens to hundreds of
bytes), which is why the mutation pages matter.

## Heuristic

Treat in-tree audits as **already-legal candidates**. The mutation
harness invents illegal gravity probes; the audits tell you which
probes the compiler can actually emit. Rank them on the served
codec. Do not keep “lean” because raw is smaller.
