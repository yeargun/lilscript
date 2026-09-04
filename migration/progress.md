# Migration progress

Parent: [index](index.md). Plan: [009](009-phases.md). Narrative and evidence: [status](status.md).

**This file is the authoritative state.** `status.md` is where findings are written up at length; it
is long, and parts of it went stale while the work moved. This is the short table you open to answer
"where are we", and **it is updated in the same commit as the work it describes**.

Updated 2026-09-04, branch `migration/target-tree`, 37 commits ahead of `main`.

---

## Where we are in one line

Phase 0 is green except the frozen baseline; **phase 1 is complete and gated**; phase 2 has its block
type and the first statement kind on a node; phases 3–8 are not started, and phase 7 has its
measurement.

**The gate is behaviour and end-state compression, not byte-identity** — revised by the owner
2026-09-04, see [D3](001-directives.md#d3) and [009](009-phases.md#the-gate-vocabulary). A step may
move bytes; it may not break a program. Only the *finished* migration has to be same-or-better on
Brotli and on compile time.

| Phase | State | Gate met |
|---|---|---|
| 0 — repair the instrument | **6 of 7 items** | — |
| 1 — the tree exists, proved against the incumbent | **complete** | BEHAVIOUR + NEUTRAL + witness (byte-identical, as it happens) |
| 2 — statements, functions, module | **2a, 2b landed; statement tree started** | BEHAVIOUR + NEUTRAL |
| 3 — the tree becomes authoritative | not started | — |
| 4 — deliver the facts | not started | — |
| 5 — naming moves post-layout | not started | — |
| 6 — the fold groups | not started (census taken) | — |
| 7 — candidate derivation and budgets | not started (**premise measured**) | — |
| 8 — retire the text layer | not started | — |

---

## Phase 0 — repair the instrument

| Item | State | Evidence |
|---|---|---|
| 0.1 assertions are real | **landed** | `[profile.release]` + `[profile.release-assert]` — `fe51558` |
| 0.2 ports are a gate | **landed** | `finer/tools/portgate.mjs`; posthoglil `trust: ok` — `fe51558` |
| 0.3 differential is generative | **landed** | `--random-seed`, seed printed before work — `fe51558` |
| 0.3b domain reaches classes/closures | **landed** | reference interpreter models instances, `super`, `this`, lexical capture — `6d0a741` |
| 0.4 baseline frozen across 61 configs | **not started** | needs a pool run and F2 |
| 0.5 live wrong programs | **6 of 9 fixed** | table below |
| F1–F5 fleet gaps | **landed** (F2 partial) | `fleet-tests.mjs` still to promote |

### Live wrong programs

| # | Defect | State |
|---|---|---|
| 1 | `extern` rewritten by source spelling (105 names) | **open** — wants the declared host binding ([003](003-target-representation.md)); a hard error breaks ports that rely on the table |
| 2 | closure `this`/`arguments` rebound by arrow spelling | **open** — port fix shipped; the compiler fix is a fleet-rule change, and [004 §2](004-legality-by-construction.md) records why the obvious fix is wrong |
| 3 | `lilscript.toml` ignored for a bare relative filename | fixed — `config_search_parent` |
| 4 | `charCodeAt` out of range yields `NaN` not `0` | fixed — no longer elidable |
| 5 | `preset = "none"` deletes a live binding | fixed — **two** fold miscompiles, `152b830` |
| 6 | `JS.number(x["length"])` loses its `ToNumber` | fixed — at the family's admission predicate |
| 7 | `optional_constructor_callback` fails to compile | fixed — `5fc2aac` |
| 8 | local-phi region duplicates a side effect | fixed — `d07a529`, found by 0.3b |
| 9 | at `optimization_level = 15`, a `\|\|`/`&&` chain loses one side-effect call | **open** — found by the probe's config matrix; pre-existing; needs its surrounding program, minimisation in progress |

Items 1 and 2 are language/fleet-rule decisions, not bug fixes, and neither blocks a later phase.
Item 9 is a real miscompile in the search and is the next thing to fix.

---

## Phase 1 — complete

| Item | State | Evidence |
|---|---|---|
| every kind retains its grammar operands | **landed** | arity table + pin test — `0570fea`, `0081beb` |
| one owner for the child list | **landed** | three `Option<Box<Self>>` fields became projections — `e7fa02f` |
| `code` derived by the printer, not authored | **landed** | `render` owns every kind but `Atom`/`Raw` — `b9ebd20`, `e7fa02f`, `697c735` |
| render options are a printer parameter | **landed** | `JsRenderOptions`; 57 hand-threaded copies removed — `697c735` |
| twin witness + negative control | **landed** | `LILSCRIPT_TWIN=1`; corruption caught 53 of 72 — `b31c5d6` |

**Gate:** BEHAVIOUR + NEUTRAL, met with room to spare — 44 of 44 artifacts came out byte-identical
across cnlil, posthoglil and markedlil, all three clean under `LILSCRIPT_TWIN=1`, compile time
+0.3% / +0.4% / +0.006%. Identity is reported because it was achieved, not because it was required.

Three latent defects fell out of doing it: `NullNormalized`, the `Member`/`Index` tag collision, and
an operandless `>>>0` `Binary` node — each invisible while `code` was authoritative.

---

## Phase 2 — half done

| Item | State | Evidence |
|---|---|---|
| 2a `JsBlock` alias, 63 signatures | **landed** | pure rename — `404ec93` |
| 2b real type, escapes named | **landed** | no `DerefMut`; `truncate`/`pop`/`remove`/`insert_str`/`replace_range` are named methods — `292803b` |
| 2b loop-keyword census as counters | **landed** | was a full rescan twice per loop — `292803b`, bounded in `00206f1` |
| **statements and module as a tree** | **started, 10 kinds** | bindings, `return`/`throw`, loop control, `import`/`export`; the emitter still writes the rest as text |
| block termination is a fact, not a text read | **landed** | 8 `ends_with(';')` sites → a maintained flag, witnessed — `afacdc1` |

`00206f1` is worth reading before the next perf change: the first counter implementation was
byte-identical and **23% slower**, and only a stopwatch could have caught it.

### The number phase 2 is driving down

Phase 1 worked because the gap was written down as a table and closed one row at a time. Phase 2's
equivalent is the count of places that assemble a statement out of text fragments instead of building
a node. It is static and `grep`-able, so it cannot drift:

    grep -c 'out\.push_str('  src/codegen_ir_js.rs     # fragment appends
    grep -c 'out\.push_statement(' src/codegen_ir_js.rs # statement nodes
    grep -c 'out\.\(truncate\|pop\|remove\|insert_str\|replace_range\)(' src/codegen_ir_js.rs

| | at 2b | now |
|---|---:|---:|
| fragment appends (`push_str`) | 324 | **251** |
| statement nodes (`push_statement`) | 0 | **22** |
| escapes into emitted text | 26 | **7** |

`JsStatement` has ten kinds — `Declaration`, `DeclarationGroup`, `Binding`, `Return`, `Throw`,
`Break`, `Continue`, `Import`, `Export`, `If` — and each holds its **children**, not their text.
`JsStatementOptions` mirrors `JsRenderOptions`: a printer setting the search varies belongs to the
printer, never to the node. Phase 3 can delete `code` when
the fragment column reaches the handful that genuinely emit sub-statement syntax.

Each family moved so far has found a disagreement the text was hiding: two `return` sites differed on
stripping an outer grouping, and two `export` emitters differed on the trailing `;`. Neither was a
bug, and neither would have been found by reading — they only show up when one node has to render
both.

---

## Phases 3–8 — not started

| Phase | Blocking on | What is already known |
|---|---|---|
| 3 tree authoritative, delete `code` | phase 2's statement tree | 22 folds (G1, G2) become unreachable |
| 4 deliver the facts | 3 | annotations, `NodeId` provenance |
| 5 naming post-layout | 2–4 | the largest single lever (katexlil identifier stream, +2,113) |
| 6 fold groups | 3 | **census taken**: 53 of 128 folds never fire; worth ~3% of CPU |
| 7 candidate derivation and budgets | — | **premise measured**: L11 beats the shipped L13 on 3 of 4 ports, on both bytes and time |
| 8 retire the text layer | 3–7 | 47,118 lines |

---

## Measurement facts that should steer the remaining work

Taken this session, on the pool, and they contradict two things the plan assumed:

- **Cost model.** One posthoglil build, CPU across threads against 31.2 s wall: `codec` **87.7 s**,
  `emit` **60.1 s** over 1,246 emissions, `peephole` 8.3 s, `analyze` 8.3 s, `lex` 3.1 s,
  `optimize` 1.3 s. The candidate search's own scoring dominates the text layer by an order of
  magnitude — so phase 7 is where compile time is, and phases 3 and 6 are correctness work whose
  speed benefit is second-order.
- **`take_trailing_expression_statements` is not a superlinearity worth fixing.** [009](009-phases.md)
  called it one; measured, it is 5.1 ms of 31.2 s — **0.016%**. Retracted there.
- **Effort is not monotone downward either.** Level 11 is smaller *and* faster than the shipped level
  13 on cnlil, posthoglil and mobxlil; markedlil prefers 13. mobxlil pays 28% of its compile time for
  64 bytes it does not get.

---

## The loop

Owner directive: iterate on **one small library**, and keep the big ports for occasional background
verification that never blocks editing.

    # inner loop -- 4.6 s, both optimization lanes, diffed against a golden
    cd ~/probelil && LILSCRIPT_COMPILER=../lilscript/target/release/lilscript \
        node scripts/build.mjs --compile

    # cheap gates
    cargo test --release --lib                  # 1,706 tests
    node finer/tools/workers.mjs check          # 144 case-lanes on the pool, ~11 s

    # occasional, in the background, never blocking
    node finer/tools/workers.mjs build --ports markedlil,zodlil --compiler <path> --dist-dir <dir>

`~/probelil` is not a port of anything. It is a small library whose source deliberately exercises
every language feature, built at `preset = "none"` *and* at the shipped config and diffed against
`expected.out`, so a wrong program at either level fails in seconds instead of in a 130-second port
build. cnlil, the smallest real port, takes 76 s here; the probe takes **4.6**.

Writing it found six things the language does not have — no hex literals, no `do`/`while`, no ternary
`?:` (the compiler *emits* them; you cannot write one), `error` is reserved, catch bindings are typed
and must be `auto` or `JsValue`, struct literals are positional while record literals are named — and
one trap worth more than all of them: copied from cnlil, its config carried `strip_console = true`, so
every `print` was stripped and the artifact ran clean while asserting **nothing**. That is the third
convincingly-empty pass this session has produced from an inherited or discovered config.

    # and the sharper one: 18 configurations, one answer, 43 s
    cd ~/probelil && node scripts/configs.mjs

`scripts/configs.mjs` builds the probe under 18 configurations — levels 0/5/9/15, `candidate_search`
off/always, three cost models, three priorities, the peephole disabled, both function spellings, beam
1 and 32, and the phi-region option that carried live-8 — and demands they all print the same 32
lines. A feature-dense program has exactly one correct output, so **any configuration that disagrees
has found a wrong program**, which is precisely how live-8 announced itself. The artifacts range 2,522 B
to 3,309 B, so those configurations really are emitting different programs.

The probe also carries a regression block for every defect that has shipped: live-8's side-effecting
selection (asserting *two* probe calls, the only possible answer), `this` escaping its object through
an arrow, bug 7's generic-with-a-func-typed-parameter, the two folds that deleted live bindings, and
`charCodeAt` out of range where `NaN|0` must be 0.

Do **not** build the 27-port fleet.
