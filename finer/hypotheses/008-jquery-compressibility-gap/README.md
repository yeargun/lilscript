# 008 — Why jQueryLil loses to `jquery.min.js` on Brotli

> **CORRECTION (added after [012](../012-port-scoreboard/README.md)).** Everything below measures
> `benchmarks/popular/ports/jquery` — the in-repo *benchmark* port. That is **not** the shipped
> library. The shipped `jquerylil` sibling repo is a different, better program:
>
> | artifact | raw | gzip-9 | Brotli-11 | vs official |
> |---|---:|---:|---:|---:|
> | `jquerylil` dist at **HEAD** | **83044** | 31530 | **28225** | **+780** |
> | `jquerylil` dist in the **working tree** (uncommitted) | 95435 | 35224 | 31483 | +4038 |
> | in-repo benchmark port (analysed below) | 89953 | 34132 | 30555 | +3110 |
> | official `jquery.min.js` | 87533 | 30336 | 27445 | — |
>
> Two things follow. **(a) The real gap is +780 Brotli, not +3110**, and the shipped library already
> *beats* official on raw by 4489 bytes. **(b) There is a live 3258-byte Brotli regression sitting
> uncommitted in `jquerylil/dist/`** — those files were regenerated at 03:43 on 2026-08-31, before
> this workstream began at 08:14, so it is not from these changes, but it should not be committed.
>
> The structural diagnosis below **does transfer** to the shipped library — the census run against
> `jquerylil` HEAD is at the end of this document — but the property-mangling *ceiling* was computed
> on the benchmark port and would need redoing against the shipped artifact.

**Status: DIAGNOSED. Cause is structural, not a missing peephole.**

## The gap

| artifact | raw | gzip-9 | Brotli-11 | raw→Brotli ratio |
|---|---:|---:|---:|---:|
| official `jquery.min.js` (Terser) | 87533 | 30336 | **27445** | 3.19x |
| jQueryLil, level 13 | 89858 | 34197 | **30651** | 2.93x |
| jQueryLil, level 15 | 92706 | 33713 | **30225** | 3.07x |

We are **+3206 Brotli bytes (+11.7%)** behind at the default level. The docs already concede this
(`docs/knowledge/evidence/jquery.md`: *"the public artifact still does not beat official
`jquery.min.js` on Brotli"*); this hypothesis asks *where the bytes are*.

**The shape of the gap is the finding: raw is only +2.7%, but Brotli is +11.7%.** Our artifact is
nearly the same size and compresses substantially worse. That rules out "we emit too much code" as
the main story and points at the *texture* of what we emit.

## Ablations (level 13, canonical codecs)

| variant | raw | gzip-9 | Brotli-11 | vs base |
|---|---:|---:|---:|---:|
| base | 89858 | 34197 | 30651 | — |
| no `string-array-packing` | 89858 | 34197 | 30651 | **byte-identical** |
| no `property-mangling` + `mangle.properties=false` | 89858 | 34197 | 30651 | **byte-identical** |
| no `string-pooling` | 89999 | 34178 | **30601** | **−50 Brotli, +141 raw** |

Two of these are more interesting than the numbers suggest:

1. **Property mangling is a complete no-op on jQueryLil.** Turning it off — both the compression
   decision and `mangle.properties` — produces a *byte-identical* artifact. jQueryLil ships with
   property mangling nominally enabled and gets nothing from it, because its internals are dynamic
   `JsValue`/ordinary-object facades that the compiler cannot prove it owns. A property census
   confirms it: our artifact still carries `length` (163), `nodeType` (60), `parentNode` (31),
   `toLowerCase` (24) as plain dotted names, exactly like the official one.
   This is objective.md point 6 in its purest form — **the loss is in the `.lil` source's typing, not
   in the compiler's mangler.**
2. **String pooling is Brotli-negative here.** Disabling it costs 141 raw bytes and saves 50 Brotli
   bytes: pooling replaces repeated literals with fresh identifiers, and a repeated literal was
   nearly free to Brotli already (it is an LZ match) while a fresh identifier is a new token.
   The admission threshold `string_pool_minimum_savings` **is** already objective-scaled
   (`src/config.rs:249` — Raw 1, Gzip 4, Brotli 8), so the question is not whether it knows about
   the objective but whether **8 is high enough** under Brotli. The ablation says no. The search
   does treat this as a scored axis, and [009](../009-search-starvation/README.md) found that axis
   **starved** — the compiler wants to tune this and never gets the budget.

## Compressibility census

| metric | jQueryLil L13 | official | delta |
|---|---:|---:|---:|
| raw bytes | 89858 | 87533 | +2.7% |
| distinct 8-grams / total | 0.695 | 0.643 | **+8.1%** |
| distinct 16-grams / total | 0.918 | 0.889 | +3.3% |
| byte entropy (bits) | 5.420 | 5.263 | +3.0% |
| identifier occurrences | 19133 | 16719 | **+14.4%** |
| distinct identifiers | 1275 | 1056 | **+20.7%** |
| 1-char identifier occurrences | 9262 | 8239 | +12.4% |
| **2-char identifier occurrences** | **3364** | **1576** | **+113%** |
| `;` share of file | 1.54% | 0.72% | **+114%** |
| `=` share of file | 4.77% | 3.50% | +36% |
| `,` share of file | 4.66% | 3.94% | +18% |
| string literals (total / distinct) | 949 / 379 | 958 / 372 | ~equal |

## Findings

1. **We emit ~2400 more identifier occurrences and 219 more distinct identifiers than Terser for the
   same library.** More distinct names means more unique 8-grams, which is exactly what the
   compressibility census measures. This is the dominant term.
2. **We emit more than twice as many semicolons and 36% more `=`.** That is the fingerprint of
   SSA temporaries surviving into the output as their own statements, where Terser has fused them
   into larger expressions. `;` at 1.54% versus 0.72% of the file is not a rounding difference — on
   an 89 KB artifact it is roughly 730 extra statement terminators, each dragging a name and an `=`
   with it.
3. **Twice as many two-character identifiers.** Partly a consequence of (1) — more live names
   exhaust the one-character alphabet sooner. jQueryLil's config sets `local_name_reserve = 8`,
   which is unusually small (the repo default is 16, most ports use 48), and worth revisiting.
4. **The character-frequency mangling that Terser uses visibly, we do not select.** Terser's locals
   are `e`, `t`, `n`, `r`, `i` — its `compute_char_frequency` reinforcing the letters already common
   in the file. Ours are `a`, `b`, `c`, `d`, `e` at every level tested, i.e. the canonical base54
   order, even though jQueryLil enables `entropy-aware-mangling` and `src/compiler.rs:3625` does
   build frequency, contextual, and keyword alphabets as scored candidates. Either the canonical
   alphabet genuinely wins on this artifact, or the alphabet family is being starved of emission
   budget on an artifact this large. **That question is decidable from the compiler's own
   `starved_emission_families` telemetry and should be answered before anything is changed.**
5. Our byte entropy is *higher* than the official artifact's (5.420 vs 5.263) — the opposite of what
   entropy-aware mangling is supposed to achieve. The largest single contributor is `f` at 3.88%
   versus 1.87%, traceable to a run of pooled-string bindings all named `<letter>f`
   (`Hf`, `If`, `Jf`, `Kf`, `Lf`, `Nf`, `Pf`) — i.e. finding (2) and finding (5) share a cause.

## How much is property mangling actually worth? (measured ceiling)

Finding 1 says property mangling is a no-op on jQueryLil because everything escapes. The obvious
conclusion — "type the `.lil` internals and unlock it" — is a large source project, so it was
priced before being recommended.

`LILSCRIPT_TRACE_PROPERTY_ESCAPE=1` reports the escape classification directly:

| port | local-only | typed | untyped | key-opaque receivers |
|---|---|---|---|---|
| jQuery | 0 keys | 0 keys | **414 keys / 9657 B** | 544 |
| motion | 0 | 0 | 244 keys / 6654 B | 584 |
| monaco | 0 | 0 | 189 keys / 3415 B | 290 |
| preact, immer, redux-toolkit, zod, acorn | 0 | 0 | **0** | 0-40 |

The well-typed ports have **no surviving property names at all** — a local struct is flattened to
array slots long before mangling is reached (`Point{a,b}` compiles to `[a,c]` with `a[0]`/`a[1]`,
which is the Closure-ADVANCED object→array trick the objective asks for, already working). So
`local-only 0 / typed 0` is not a broken analysis: those records were eliminated upstream, and only
escaping receivers leave keys behind.

Then the ceiling itself. Renaming **every** dotted property name in the shipped jQueryLil artifact —
an unachievable upper bound, since most of those names are real DOM and public-jQuery API — gives:

| artifact | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| jQueryLil, level 13 | 89849 | 34166 | 30593 |
| jQueryLil, every property mangled (ceiling) | 77308 | 32357 | **29316** |
| official `jquery.min.js` | 87533 | 30336 | **27445** |

**Even at the unachievable ceiling, jQueryLil is still +1871 Brotli behind official.** Property
mangling is worth **at most 1277** of the 3148-byte gap — 40% — and realistically far less: only 47
of 471 dotted names (473 bytes) are certainly jQueryLil-internal rather than names official jQuery
also uses.

Note the ratio: mangling every property saves **12541 raw** bytes but only **1277 Brotli**, roughly
10:1. Property names are long and highly repetitive, so the compressor was already handling them
nearly for free. This is the objective's own point about raw and Brotli being different games,
measured.

**Consequence: the ranking below is wrong as originally written.** Typing the jQueryLil source was
listed last as "the largest available win"; it is in fact bounded at 1277 Brotli bytes, while the
**1871-byte structural residue is the larger prize** and is compiler work rather than source work.
The list is reordered accordingly.

## Ranked next actions

1. **Answer the starvation question** (finding 4) from `--explain json`'s
   `scored_emission_families` / `starved_emission_families`. **Done —
   [009](../009-search-starvation/README.md): the search is starved at every level, and feeding it
   was worth −58 Brotli on jQuery.**
2. **Raise `string_pool_minimum_savings` for the Brotli objective.** It is already objective-scaled
   (Raw 1, Gzip 4, Brotli 8) but 8 is measurably too low: disabling pooling outright is worth 50
   Brotli bytes on jQuery. [010](../010-string-pool-alias-pricing/README.md) recovered 3 of those by
   fixing the alias-width model; the rest is threshold calibration.
3. **Attack the temporaries** (finding 2). 730 extra statements' worth of `name=...;` is the largest
   structural term in the gap. This is `local_name_coalescing` / SSA-destruction quality, not a
   peephole.
4. **Raise `local_name_reserve` from 8** on the jQuery port and re-measure (finding 3) — a config
   experiment, no compiler change.
5. **Type the jQueryLil internals so property mangling can fire** (finding 1). Now known to be
   **bounded at 1277 Brotli bytes** and realistically much less, against a large `.lil` rewrite.
   Ranked last on that basis, not first.

## Census against the *shipped* jQueryLil (added with the correction above)

| metric | `jquerylil` HEAD | official | in-repo benchmark port |
|---|---:|---:|---:|
| raw | **83044** | 87533 | 89953 |
| Brotli-11 | 28225 | **27445** | 30555 |
| distinct 8-grams / total | 0.703 | **0.643** | 0.693 |
| byte entropy | 5.258 | 5.263 | 5.420 |
| identifier occurrences | 17990 | **16719** | 18988 |
| distinct identifiers | 1198 | **1056** | 1270 |
| `;` share of file | 1.53% | **0.72%** | 1.58% |

The shipped library has **closed the entropy gap entirely** (5.258 vs 5.263 — the benchmark port is
at 5.420), and it emits **4489 fewer raw bytes than Terser**. What it has not closed is
**compressibility**: 8-gram uniqueness is still 0.703 against 0.643, identifiers are +7.6%, and
**semicolons are still at 2.1x Terser's share**.

That is the whole remaining +780 bytes, and it is one mechanism: surplus statements carrying surplus
names. jQueryLil is emitting *less code* than Terser in a *shape that compresses worse*. Closing the
semicolon gap is the plausible path to a win, and it is compiler work — SSA destruction and
`local_name_coalescing` quality, not a peephole and not source typing.
