# 011 — Phase 5, step by step: naming moves onto the tree

[009](009-phases.md) gives phase 5 one sentence of mechanism and three gates. This is the
mechanism, in the order it lands, with what each step measures. Written 2026-09-05 after the
gate instrument ([progress](progress.md), *5-gate*) landed on the incumbent.

## Where names are today

Measured on `migration/target-tree` at the 5-gate commit:

| site | count | what it holds |
|---|---|---|
| `JsExpression::atom(` | 101 | an identifier or literal, **as rendered text** — no binding identity |
| `JsExpression::raw(` | 124 | formatted text, some of it embedding names (`delete {name}[..]`) |
| `JsStatement::Binding { name: String }` | 22 | a declaration, by spelling |
| `JsDeclarator { name: String }` | 15 | same, in a group |
| `JsStatement::Function { head: String }` | 7 | the whole head as text: name, parameters |
| `JsStatement::Class { head: String }` | 2 | same |
| `context.value_name(` | 50 | the lookup every rendering goes through |

Names are assigned **before rendering**, per function, in `LocalNames::new`: a pool cloned from
the top-level mangler with the enclosing scopes' names reserved and the counter rewound, then
requests in this order — the `$state` name when the function is a state machine; captured storage
locals; coalesced value colors (preferred and idiom spellings claimed first, the rest in color
order, or by weight under `frequency_order_local_names`); the remaining values (emitted first, then
by descending use, then by id); the function's locals; the parallel-copy temp. Module-level names
come from `top_level_mangler` in emission order of the top-level walk. **The trace prints exactly
this** (759 requests per emission on the probe).

So today the order is fixed by construction, not chosen; the pool restarts per function (names can
only converge by coincidence of frequency); and nothing downstream of the print boundary can rename
soundly, because the tree does not know which atoms are the same binding — which is why
`rename.rs` re-derives binding identity from tokens and gives up (`rename_ambiguous`) whenever two
declarations spell alike.

## The steps

Each step is one commit on the branch, verified by the recipe, and the trace gate applies from the
first: a step that changes the `(order, name)` sequence without declaring it fails.

### 5.1 — Binding identity on the tree (byte-neutral, trace-neutral)

`Bind(u32)`, dense per emission, allocated where the incumbent *requests* a name — the mangler
call sites — so the id order is the request order and the trace can print it. Every identifier
atom that names a binding carries its `Bind` beside the rendered `code`
(`JsExpressionRoot::Atom` splits into `Atom` and `Name(Bind)`); every declaration form
(`Binding`, `Declarator`, function and class heads, catch and loop-head parameters, import and
export locals) records the `Bind` it declares. A per-emission `Spellings: Vec<String>` indexed by
`Bind` is the one source of the text. `value_name()` returns both.

The witness for the step: the twin asserts, at every block boundary, that each `Name(bind)` atom's
`code` equals `spellings[bind]`, and the census counts atoms that spell an identifier without a
`Bind` (the residue). Gate: 0 byte diffs, 0 trace diffs, 1,7xx tests, ports byte-identical.

### 5.2 — Spelling becomes a side table (byte-neutral, trace-neutral)

`code` for a `Name` atom is no longer stored at construction: it is derived from the table on
render, through the same rebuild path the witness already exercises (`rebuilt()` proves every
node reproduces from its tree). A rename is then a table write plus a re-spell of the functions that
contain the binding — the print memo key from [006](006-candidate-derivation.md): `(node, digest of
the spellings reachable from the subtree)`. The 124 `raw(` sites that embed a name are the residue
this step drives down; a function that still has one is marked *unrenameable* and every later
ordering leaves it alone — the same conservative rule `rename.rs` applies today, now decided from
the tree instead of from a failed token resolution.

### 5.3 — `NameOrdering::EmissionWalk` (byte-neutral, trace-neutral, the anchor)

The first *post-layout* assignment: after the module is laid out and every function is on the
tree, a pass walks it in emission order, allocates spellings for `Bind`s in the order the walk meets
their declarations, from a pool that reproduces the incumbent's reservations (enclosing scopes,
cross-scope reuse, precise shadowing), and writes the table. Its trace must equal the incumbent's
and its bytes must equal the incumbent's on the 61 configs. Only then does the pre-render naming in
`LocalNames::new` stop being the producer of spellings (it still produces `Bind`s and coalescing).

Lands as a `CompressionDecision` with a registry row, **default off**; the decision is flipped on
by default in its own commit once the fleet says byte-identical.

### 5.4 — The orderings that change bytes (each its own A/B)

- `NameOrdering::FrequencyDesc` — one module-wide pool; spellings by descending use across the
  whole artifact. The owner's finer 059 measured this axis on the fleet: *name convergence loses to
  frequency, and the reason is entropy* — so this is the ordering expected to win, and
  `frequency_order_local_names` (per function) is its incumbent approximation.
- `NameOrdering::IdiomConverged` — ports `rename.rs`'s idiom preference (`idiom_conversion_groups`,
  finer 060: two wins, two ties, no regression on the fleet) onto binding identity, where
  `rename_ambiguous` cannot happen. This is the step that lets `rename.rs` delete.
- `ReservationMode::Precise` — a separate axis: reserve only names actually referenced across the
  scope boundary (today: `precise_cross_scope_shadowing`), measured on its own.

Each is a scored decision, default off, with its own trace baseline declared in the commit (the
gate for a new ordering is *its* trace, stable across thread counts — not the incumbent's).

### 5.5 — Deletions

`rename.rs`, `binding.rs`'s `BindingResolution`, the third `Mangler`, `rename_ambiguous` — after
`IdiomConverged` matches or beats the text pass on the fleet (katexlil's +2,113 identifier stream
is the number to move). Until then the text pass runs after the tree pass, and the census reports
what it still changes.

## What this step list does not do

It does not change the search: naming stays a candidate axis scored by the codec, and the
lexicographic tiebreak on artifact text stays ([006](006-candidate-derivation.md), mechanism 3). It
does not touch `plan_identity` or the budget reservation. And it does not promise the +2,113 —
it makes the pass that could win it sound.
