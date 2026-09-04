# 003 — The target representation

Parent: [index](index.md). Directives: [D1](001-directives.md#d1--correctness-by-construction-not-by-conditional),
[D2](001-directives.md#d2--one-fact-one-owner).

Three independent designs were produced and scored by three independent reviewers. The compression
reviewer picked one, the architecture reviewer another, the performance reviewer the third — and each
then grafted the other two onto their winner. That is not a deadlock. It is evidence that the three
were answering **different questions**:

| | Answers | Contributes |
|---|---|---|
| **Terrace** | which level owns which fact | the ownership boundary |
| **Weft** | how nodes are stored, shared and derived | the arena substrate |
| **Ratchet** | how to get there proving byte-identity at every commit | the migration method |

The design below takes all three. They are orthogonal in two of the three pairings, and the reviewers'
own grafts say so. **In the third they genuinely conflict**, and that conflict — Terrace materializing
a second tree against Weft's requirement that no second tree ever exists — is resolved below rather
than papered over. Two independent synthesis passes found it and resolved it identically: keep the
boundary, drop the storage.

---

## Why the current type forces this

`JsExpression` is not a tree with a cached string. It is a **string that drags an advisory partial
tree**, and only 3 of its 10 root kinds keep their operands at all — `call`, `member`, `index`,
`conditional`, `nullish` and `comma` all build through `grouped()` and set every child link to
`None`. `impl Deref<Target = str>` and `impl Display` then make any `&JsExpression` silently usable
as `&str`.

Its cost, measured from the definition:

```
String(24) + Option<String>(24) + Option<String>(24)
  + 3 × Option<Box<Self>>(24) + precedence/root(~10)   ≈ 106 bytes inline per node
```

…and `code` on **every node holds the entire rendered text of the subtree beneath it**.

Then the hottest constructor in the emitter:

```rust
fn binary(op: IrBinaryOp, mut lhs: Self, mut rhs: Self) -> Self {
    if op == IrBinaryOp::Add {
        if is_rendered_string_literal(&rhs.code) { … }      // ← semantic decision from rendered text
    }
    if … && rhs.is_constant_literal() && !lhs.is_constant_literal() { … }   // ← and again
    let operands = (Box::new(lhs.clone()), Box::new(rhs.clone()));          // ← deep clone, each carrying its subtree's text
    …
    Self::grouped(format!("{lhs}{}{rhs}", binary_operator(op)), …)          // ← a third copy containing both
}
```

A left-leaning chain of depth *d* over leaves totalling *L* bytes therefore holds **≈ d·L bytes of
`String`**, and pays it again in clones.

That is the measured superlinearity. Predicted from the structure: n^1.79. Measured on a clean
no-search build: 48 KB → 380 ms, 98 KB → 1,301 ms, **3.43× for 2.05× ≈ n^1.72**. The agreement is
the point — this is not an incidental hot spot to patch, it is what the representation *is*.

**Target: 16 bytes per node, no text, children by id.**

---

## The organizing rule

Everything below follows from one statement, which is the type-level form of a boundary the codebase
already declares in prose:

> **Profitability may only choose among programs legality admits, and the type system enforces the
> direction of that dependency.** No value derived from `IrJsOptions` may reach a token stream except
> through a function that also sees the node's legality fields.

`JavaScriptCompilationContract` is already documented as *"immutable legality input… intentionally
separate from profitability"*, and `JavaScriptOptimizationObjective` as being able to *"choose among
programs admitted by the compilation contract, but cannot alter that contract."*
[Live-2](002-the-instrument.md#live-2) is precisely `IrJsOptions::function_spelling` reaching the
emitter **without passing that boundary**. The design makes the boundary a type rather than a comment.

---

## Three levels, two materializations

The first draft of this design had two levels. Two independent syntheses, working from the same
research, found the same genuine conflict: **materializing a second boxed tree per candidate destroys
the sharing argument that makes derivation cheap.** Both resolved it the same way — keep the
*boundary*, drop the *storage*.

| Level | Materialized? | Owns | May not know |
|---|---|---|---|
| **L0 `Shape`** — shared base arena + per-candidate overlay | **yes** — `Arc<Base>` + one overlay per candidate | *what program*: operators, control flow, bindings, scopes, calls, values, latch, receiver use, calling convention, obligations | any name, paren, quote, terminator, brace, operator polarity; whether declarations are joined; whether an arrow body is concise |
| **L1 `Spell`** — a `SpellPolicy` value plus a sparse `SortedVec<(Node, SpellChoice)>` of per-site overrides | **no** | *how written*: grouping choice, `true`-vs-`!0`, quote char, number spelling, declaration joining, concise-vs-block arrow, member-vs-index, name **slots** | the byte offsets it lands at; it never sees the output buffer |
| **L2 Print** — one `String` behind a sink | **yes**, one buffer | *token adjacency only*: the minimal separator between two adjacent token classes | everything else — it has no policy left to apply |

**`Spell` is not a tree.** It is roughly one byte of policy per decision, materialized only where a
per-site override exists. That keeps "how written" with exactly one owner while costing zero
allocations, and nothing in it can go stale.

### The Sharing Rule

The law that makes the split sound, and the generalization of two exceptions the first draft listed
separately:

> **A fact may be stored on a node only if it is a property of the *value the node computes*. Any
> fact that depends on where the node appears is a parameter of the traversal.**

This is not stylistic. The overlay shares a subtree across parents **by design**, so a slot holding a
parent-dependent fact is read by two different parents and answers wrongly for at least one. `PURE`,
`INT32`, `OWNED_SLOT` are value properties and live on the node. `BOOL_CTX` and `SINGLE_USE` are use
properties and are threaded through the traversal context.

---

## Level 0 — `Shape`: what program

Built once per IR context. Arena-allocated, id-keyed, carrying an `Origin` back into the SSA IR and a
16-bit fact header, with real lexical scopes and binding identity.

It has **no names, no parentheses, no terminators, no quotes, no braces, and no operator polarity
choices.** Those are not decisions Shape is allowed to hold.

### Ids

```rust
// Dense u32 indices into per-module arenas, assigned by one deterministic
// construction walk — never by hash-map iteration.
pub struct Node(u32);    // Node::NONE == u32::MAX
pub struct Bind(u32);    // a lexical binding. Identifiers name a Bind; spelling is a side table.
pub struct Atom(u32);    // interned literal payload: string body, f64 bits, regex source, key
pub struct Scope(u32);
pub struct Origin(u32);  // packed (FunctionId, ValueId) back into the SSA IR
pub struct List { off: u32, len: u32 }   // contiguous child run in Arena::lists
```

All of them derive `Ord`, **on purpose**. `src/codegen_ir_js.rs` carries **57 explicit sorts whose
only job is making `AHashMap` iteration deterministic**, and `src/stable_hash.rs` exists because
table iteration order is observable in the generated program. Key every emission-path map by a dense
ordered id — a `Vec<T>` indexed by id, or a sorted `Vec<(id, T)>` — and those 57 sorts and the
stable-hash types delete. Determinism stops being a discipline 57 call sites must remember and
becomes a property of the representation.

`Origin` pins its bit split explicitly and **fails closed**: any id that does not fit yields
`Origin::NONE` rather than silently wrapping and attributing a fact to the wrong node.

### The node

```rust
pub struct ShapeNode {
    op:     Op,       // u8
    aux:    u8,       // op-specific discriminant
    facts:  Facts,    // u16 inline flag word
    origin: Origin,   // u32
    a: u32, b: u32,   // children / atoms / lists — meaning fixed per Op
}                     // 16 bytes
```

`a` and `b` are a union whose meaning depends on `op`. That is a real hazard — the performance
reviewer flagged it as fatal in the design that left it implicit — so the **per-op encoding is a
single exhaustive table, and the accessors and `Op::arity()` are generated from it**, never
hand-written. A new `Op` without a table row does not compile.

### Illegal states removed by typing, not checking

**A `Place` is not an expression.**

```rust
pub enum Place { Bind(Bind), Global(Atom), Member(Node, Atom), Index(Node, Node) }
// Assign { target: Place, .. } — structurally cannot hold a Call.
```

**A sequence cannot have a hole, a conditional cannot have an absent arm.** `Seq(List)` and
`Cond { test, cons, alt }` with non-optional arms. That is not a stylistic choice: it makes the nine
folds of [G2](007-fold-disposition.md) — the repairs-for-repairs — *unreachable* rather than
unnecessary.

**A function declares how it uses its receiver.**

```rust
pub struct ShapeFn {
    params: List, body: Node, scope: Scope,
    receiver_use: ReceiverUse,   // REQUIRED. bitflags: THIS | ARGUMENTS | NEW_TARGET
    call_conv:    CallConv,      // Formals(n) | Rest | ArgumentsObject
    is_async: bool, is_generator: bool,
}
```

`receiver_use.is_empty()` is a **precondition of the arrow spelling**, enforced where the Spell node
is constructed. This closes [Live-2](002-the-instrument.md#live-2) by construction: today the check
is a conditional conjoined with `matches!(function.kind, FunctionKind::Function)`, so a closure
skips it and `function_spelling = "arrow"` ships a `ReferenceError`. A required field cannot be
conjoined with the wrong thing.

**A loop knows its latch.**

```rust
Loop { header: Node, body: Node, latch: Option<Node>, exit: Node }
```

`latch` is copied from `ControlShape::Loop { update }`, which the IR already carries and the emitter
already throws away. Two of the three shipped wrong-program folds are loop-header reconstructions
that guess the latch by scanning tokens backwards for `?`, `:` or `=>` at depth 0. Carrying the fact
closes both by construction.

**There is no `Raw` variant.** Not deprecated — absent. A construct that cannot be modelled is a
design finding, not an escape hatch. During the migration it lives behind a `cfg` feature so a
concurrent merge cannot reintroduce one, and the phase does not close until it is deleted.

### Facts

Two tiers, and the split is load-bearing.

**Inline, 16-bit flag word** — exactly eight, because these are what every rewriting pass tests in
its inner loop, and on cnlil **11,085 of 13,606 fold invocations** test one of them:

```
PURE  NO_THROW  OWNED_SLOT  INT32  NON_NULLISH  LOCAL_ONLY  SOURCE_ORIGIN  HAS_OBLIGATION
```

**Everything else in `Arc`'d side tables keyed by `Origin`.** The pattern is already proven here:
`Arc<IntegerValueAnalysis>` is built once behind a `OnceLock` and shared across all ~300 candidate
emissions. This generalizes it to the twenty facts that currently die at the print boundary
([010](010-what-this-unlocks.md)).

Two rules keep it sound:

- **`HAS_OBLIGATION` is structural.** Today `PreserveJavaScriptBitOrZero` is verified by counting
  `|` followed by `0` token pairs and comparing `observed >= expected` — it cannot distinguish a
  source obligation from a compiler-generated normalization. A flag plus a sparse kind table can.
- **The Sharing Rule applies here first.** `BOOL_CTX` and `SINGLE_USE` are properties of a *use*, so
  they are traversal parameters, never node slots. Use counts are recomputed per pass by one walk
  into a pass-local `Vec<u32>` indexed by `Bind` — O(nodes), never cached, never in a side table.

---

## Level 1 — `Spell`: how it is written

A total pure function of a `Shape` node, a traversal context and a `SpellPolicy` — not a stored
tree. Per-site choices the beam scores live in a sparse `SortedVec<(Node, SpellChoice)>`.

The split earns its keep three ways, and the measured option census is what makes it decisive:

**Of the 80 `IrJsOptions` fields: 20 are printer-only, 46 are bounded reversible local rewrites, 3
are structural, and 11 are ABI/legality and never scored.** A 66:3 ratio of "re-derivable cheaply"
to "needs a different base".

1. **Every option sits at exactly one level.** Spelling options re-run `Spell` over an unchanged
   `Shape`; local options apply an overlay; only the 3 structural ones rebuild a base.
2. **It tells you where each fold goes.** [G1](007-fold-disposition.md) is Spell policy;
   [G9](007-fold-disposition.md) is Shape transforms. No judgement call.
3. **`Shape` stays canonical**, so two candidates differing only in spelling share their whole
   `Shape` by identity.

There is a sharp corollary in that census. On markedlil's real production search, 56 admitted
emission axes reduce to 53 printer-or-local against 3 structural — and **all three structural axes
appear in that run's starved-families line.** The search exhausted its budget before it ever tried
the only axes that actually need a rebuild. Roughly 16 realized bases would serve 270 emissions.

**One producer only.** `Spell` is computed from `Shape` and from nothing else, ever.

---

## Scored variants: transforms propose, the beam disposes

A backend that always emits the shape it thinks is best **will regress ports**. Several folds today
ship *scored alternatives*, and the codec — not the compiler — picks. That must survive.

```rust
pub trait ShapeTransform {
    fn sites(&self, m: &Shape) -> Vec<SiteId>;
    fn apply(&self, m: &mut Shape, subset: &SiteSet);
    fn polarities(&self) -> &'static [Polarity];
}
```

A transform **proposes sites**; the beam scores subsets and polarity. This type must exist before the
scored fold families (G9, G10, G13) migrate — otherwise "port the fold" silently becomes "fix the
choice", and the acceptance check in [007](007-fold-disposition.md#what-delete-has-to-prove) —
unchanged distinct-candidate count — is what catches it.

---

## Candidate derivation

`Shape` is built once per IR context. A candidate is an **overlay**: an append-only layer read
through a single bounds compare, never a cloned tree.

The traversal contract is what makes it pay:

> `descend` returns the **identical** `Node` when no child changed. A pass that fires nowhere costs
> one walk and zero allocations.

Untouched subtrees are therefore shared across all ~300 candidates **by identity**, which is also
what makes the print memo work: key it on `(Node, digest of the spellings of bindings reachable from
that subtree)` — **not** a global name revision. Naming is the dominant candidate axis, so a global
revision invalidates every function on every rename and the memo does nothing.

Two invariants:

- **An overlay is derived, named, printed, scored and dropped inside one candidate evaluation.** Only
  bytes and score enter the plan registry. Otherwise the beam holds hundreds of live overlays and the
  memory story is unbounded — the performance reviewer flagged this as unspecified in the design that
  proposed it.
- **Re-entry is a caching decision only and may never change bytes.** `re_entry(from, to) -> Print |
  Shape | Base` is a pure classifier over the option tuple, and a shipped assertion re-derives
  registered tuples from their base to prove path-independence.

---

## The `extern` host binding

[Live-1](002-the-instrument.md#live-1) is not an optimizer bug. `ExternDecl` carries only `name`, and
`FunctionKind::Extern` is a unit variant — **spelling is the only identity that reaches the
optimizer**, so matching on it is the only thing the IR permits. The 105-name table substitutes for a
missing language feature.

```lilscript
extern("Math.round") pure float mathRound(float value);
extern("Math.round") pure float whateverIWant(float value);   // same binding
extern void mathRound(float v);                               // no binding -> inert, always
```

The construction that matters is not "add a field" — it is **give the rewritable case an identity
that cannot be produced from a string**, and then make the lookup total:

```rust
// src/target/host.rs — the ONLY module that can mint one.
pub struct HostSymbol(NonZeroU32);

pub enum HostBinding {
    /// The user's own declaration. The compiler knows its type and
    /// `declared_pure` and NOTHING else — no spelling, no arity, no aliasing.
    Opaque(ExternId),
    /// Resolved from the declared platform surface, never from how the user spelled it.
    Known(HostSymbol),
    Imported { module: Atom, member: Atom },
}
// ExternDecl gains: pub host: HostBinding

fn js_host_alias_spec(sym: HostSymbol) -> (&'static str, JsHostAliasConvention);  // TOTAL. No Option.
```

`Opaque` **carries no key**, so it has no path into the alias table — the table's parameter type is
`HostSymbol`. There is no check to forget. And because the match is now exhaustive over a closed
enum, **adding a `HostSymbol` without a spelling is a compile error**; today a typo in the table is a
silent miss.

The 105 spellings survive as the prelude's declared names, so every port keeps compiling and the
change is byte-identical for any module whose externs resolve the same way. A module where they do
not resolve the same way is a module that was being miscompiled. Lookup also becomes an interned
integer compare rather than up to 105 string comparisons per call site.

The mechanism already exists in the language: `ForeignImportDecl` carries a declared external name
into `JavaScriptForeignImportAbi` today.

---

## The migration method: ratchet, with a witness that actually runs

The tree is built **alongside** the existing `String`, and every constructor ends with a witness
comparing `print(&node)` against the `code` it built. Release still emits from `code` until phase 3.

For those phases a printer bug **cannot reach an artifact**. It can only fail an assertion. That is a
stronger guarantee than any measurement, and it is available for free at the start — which matters
because with the whole text layer worth 189 bytes and the noise band at −125..+30, an artifact-level
diff cannot distinguish "the tree emits the same program" from "the tree lost an optimization".

Two things make the witness real rather than decorative:

- **It is not a `debug_assert!`.** `Cargo.toml` has no `[profile.release]`, so those are compiled out
  of every build the fleet measures. The witness is env-gated (`LILSCRIPT_TWIN=1`) and runs in
  release, and it additionally rides a `[profile.release-assert]` so all 1,715 existing tests
  exercise it for free.
- **It asserts against a quirk-preserving printer.** Several current spellings are *not* pure
  functions of the options — `without_explicit_tostring` strips a `+""` suffix from rendered text,
  `is_constant_literal` matches on a rendered string. Each such quirk is a **named ledger entry**,
  default on, that retires later as its own commit with its own A/B. Without the ledger they surface
  mid-migration as unexplained byte drift with no owner.

And when the witness does fail, the printer emits a `(Node, byte_range)` sidecar so the report names
the node kind, its `Origin` and its span — not a byte offset into an artifact nobody has read. Under
`FunctionLayout::CompressionSimilarity` a one-byte divergence reorders the whole artifact, so a
diagnosable failure is the difference between a ten-minute fix and a day.

---

## What this does not solve

`Shape` gives the compiler binding identity and delivered facts. It does not give it knowledge it
never had. **[G12](007-fold-disposition.md#g12-is-the-exception-and-it-must-be-named-as-one) — class
recovery from prototype tables, 11 folds and 8,017 lines — recovers structure from *source* idiom**
that the IR never modelled, because the ports in question are untyped `JsValue` transliterations.

The tree improves how that recovery is done (binding identity instead of token guesses). It does not
remove the need for it. Typed ports do.
