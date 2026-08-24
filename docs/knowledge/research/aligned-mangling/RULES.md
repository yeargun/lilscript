# Rules for a Brotli-first emitter

Parent: [index](README.md). Companion to the older
[playbook](../brotli-global-mangle/11-playbook.md), which these extend rather
than replace. Every rule here has a measurement behind it in this folder; the
ones that contradict intuition are the ones with the most measurement.

Ranking still belongs to `lilscript-codec` on the complete artifact. These are
generation and prioritisation rules, not a substitute for scoring.

## The objective

**1. Minimise distinct spellings, not name length.**
Two live ranges that do not interfere should get the same name even when a
fresh letter is free. On LilScript's own artifacts this is worth 2.2–2.5%:
−801 on the in-tree jQuery port, −770/−776 on the published jquerylil package,
−96 on solidlil's reactive core, all behaviour-verified.
[05](05-concentration.md), [07](07-ports.md).

**2. Shortening a name that adds a new spelling can lose.**
Raw length and compressed size disagree here, and the compressed side wins.
The jquerylil rewrite is 524 raw bytes smaller and 770 Brotli bytes smaller —
the Brotli saving is larger than the raw one, because every occurrence of a
rarer name was also a more expensive literal.

**3. Judge a proposal by the command stream, not by raw length.**
Distances are 35–65% of these streams; literals are 4–26%; the whole prefix-code
header is under 2%. A change that adds distinct *distances* is fighting a much
bigger channel than one that adds literal bytes. [01](01-where-the-bits-are.md).

**4. Rank namings with the codec. No proxy survives holding raw size constant.**
Name entropy explains the direction of a large win and cannot rank small ones;
with raw size fixed, every proxy tested collapses to noise
([05](05-concentration.md)).

## The dictionary

**5. Never spell an identifier as a dictionary word.**
Any frequency, any corpus: +1,017 to +6,341 Brotli bytes. The dictionary is a
few hundred **first occurrences** per artifact — in every stream measured,
every dictionary reference is used exactly once. [03](03-dictionary-as-names.md).

**6. Spend the dictionary on first occurrences of literals you must emit anyway.**
Measured value: −9 to −15 bytes as a tie-break on pooled-string order. Worth
taking, not worth designing around. [06](06-free-order.md).

**7. Know what is actually in it before appealing to it.**
`function(`, `=function(`, `);return `, `for(var `, `}else{`, `this.`,
`.length`, `.call(`, `.push(`, `Math.`, `var `, `typeof ` are each one
reference. `let `, `const `, `=>{`, `await `, `if(`, `.prototype.`,
`constructor`, `parentNode`, `nodeType` are not there at all.
[02](02-the-hardcoded-library.md).

## Emission order

**8. Emit pooled literals in reversed-string order under a Brotli cost model.**
−50 to −70 Brotli bytes, free, legal, no raw change. Property names share
endings more than beginnings. gzip prefers alphabetical — keep it a scored
proposal, not a rule. [06](06-free-order.md).

**9. Leave function layout alone.**
Five orders across six corpora: one win of 0.5%, losses everywhere else, and
the implicit-distance rate moves by less than a point. The distance-cache lever
is real but whole-function permutation is not its handle.

## What not to generate

**10. No copy-maximising name alignment.** +49 to +673 Brotli bytes; a
bit-cost objective declines every move it would make. [04](04-alignment.md).

**11. No `function` spelling to reach the ROM**, no dictionary warm-up
preambles, no bracketised `.length`. Previously measured in
[the audits](../brotli-global-mangle/09-audits.md); the census now explains
why: the win is one reference, the cost is every later occurrence.

**12. No fresh alphabet per function.** Breaking cross-scope reuse costs
14–28 KB on minified jQuery ([02 reuse](../brotli-global-mangle/02-reuse.md)).

## Where effort pays

**13. The naming work pays where the compiler's emit is the final artifact.**
jquerylil's dist files and solidlil's reactive core: 2.2–2.5%. Anything a
bundler re-mangles afterwards — every solidlil app bundle measured — has
nothing left; rolldown and esbuild already took it. Spend the effort there on
shape instead, where the LilScript/JavaScript gap is 14.5% on one bundle pair.
[07](07-ports.md).

**14. A smaller artifact is not a better artifact until it computes the same
thing.**
On markedlil, `cost_model = "raw"` produces the smallest build of the family —
and drops the `mailto:` prefix from email autolinks, failing two CommonMark
cases the other three cost models pass. Reading the two builds side by side
shows why: the extra statement fusion let a read of a regex match group sink
past the reassignment of the variable holding the match. That is the `ident`
lane's invariant, not a cost-model bug. Every size comparison in this
repository should be paired with the port's own battery before it is believed.
[07](07-ports.md), and Phase A0 of [PLAN.md](PLAN.md).

**17. Break a codec tie with raw size.**
Under `priority = "size-first"` with a Brotli cost model, markedlil is taking
the side of a ±3-byte Brotli tie that is **920 raw bytes larger**. Raw is never
free: it is parse time, memory, and the gzip lane. When the ranked metric
cannot separate two candidates, the smaller one should win.
Measured per family in [07](07-ports.md): merging adjacent declarations is
−920 raw / −60 gzip / ±3 Brotli, while `for(;t;)` → `while(t)` (+19) and naive
call outlining (+126) are real Brotli losses the model is right to decline.

**18. The pass list is not where markedlil's bytes are.**
The obvious guess — jquerylil enables 30 compression passes explicitly,
markedlil takes the defaults — was tested and refuted: rebuilding markedlil
with jquerylil's list moves five Brotli bytes and leaves all 230 adjacent
declarations unmerged, while doubling search time. What decides that merge is
the **cost model**, not the enabled passes. Test the config hypothesis before
recommending a config change.

## How to search

**19. Factor by what a transform rewrites.**
Two knobs that touch the same bytes are one decision with several levels, not
two switches. Modelling naming as two independent switches puts the additive
fit at R² 0.55; modelling it as one four-level factor puts it at **0.9968**,
and one term (`N×O`) carried 99.4% of the apparent coupling.
[08](08-search.md).

**20. Deltas do not add across mis-factored axes; they add fine across correct
ones.**
With naming modelled as two switches, stacking independently measured deltas is
off by up to 601 bytes. With naming modelled as one decision, the same stacking
is off by 0–32 and greedy coordinate descent lands within 25 bytes of
exhaustive search over the product — at ∑|levels| evaluations instead of
∏|levels|. The expensive thing is not the search; it is searching a space that
was parameterised badly.

**21. Screen a family by its main effect over the grid, not in isolation.**
`for(;t;)` → `while(t)` costs 19 Brotli bytes measured alone on markedlil and
has a −12.9 main effect across the grid — it appears in the best point of all
four artifacts tested.

**22. A grouped factor is only as good as its level set.**
On jquery-lil-raw the two-switch design found 30 bytes that the four-level
grouped design missed, because applying both renamings composes to a naming
none of the four levels contained. Group the decision, then *generate* its
levels rather than listing the ones you thought of.

**23. Report R² and RMSE of the additive fit when a family joins the beam.**
If R² drops, the partition is wrong and the new family belongs inside an
existing one. That is a cheap, mechanical check on whether the search's
independence assumption still holds.

**24. Score the inner loop analytically; gate with the codec.**
`L = Σ −log2 p + header` computed from one parse ranks candidates at r 0.93–0.99
with mean error 0.15% of artifact size, 6–10× cheaper than q11 — in
unoptimised JavaScript against a C encoder. Use it for the beam; use
`lilscript-codec` for the finalists and the gate. [09](09-the-equation.md).

**25. Never price a change against frozen statistics.**
The gradient `∂L/∂n_s = −log2 p_s` is exact and useless: on the artifact whose
edits are renamings it is **anti-correlated** with the truth (r −0.14, mean
error 589 bytes), because renaming changes the distribution rather than the
symbols. Recompute the histograms — it is one pass — instead of stepping along
the old ones.

**26. A checker is only valid for the rewrites it was designed for.**
The resolution-sequence proof indexes bindings by declaration position, so it
reports a *reordering* rewrite as illegal even when it is correct. Composing
rewrites does not compose their proofs: prove each step against its own input,
with the check that step admits, and behaviourally test anything that has no
proof.

## How to keep this honest

**15. A proposal that claims a mechanism must show the census row where the
mechanism appears.** If it claims to help the dictionary, dictionary references
must go up. If it claims to help the distance cache, the implicit-distance rate
must move. Two ideas in this folder died on that test after looking plausible
in prose. `census.mjs` prints the row.

**16. A structural proof is not a semantic gate.**
This folder's own scope analyser produced rewrites that passed a binding-graph
check and threw `t is not defined` on all 680 marked spec cases. The behavioural
differential caught it; nothing else could have. Every naming change needs a
battery, not an argument. [07](07-ports.md).
