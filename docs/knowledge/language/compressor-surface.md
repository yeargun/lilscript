# Language surface for the compressor

Parent: [Language](README.md). Contract: [`docs/language-v0.1.md`](../../language-v0.1.md).
Objectives: [compilation objectives](../compilation/objectives.md).
Ports: [corpora](../evidence/corpora-and-lanes.md), [jQuery](../evidence/jquery.md).
Plan: [reusable-proof phase](../migration/planned-migration.md#phase-4-close-library-losses-with-reusable-proofs).

LilScript beats Terser, Oxc, and Closure ADVANCED when the program is **written
in LilScript**. It does not beat them by transliterating JavaScript into
`JsValue` and hoping a fold recovers what the type system never saw.

Forked libraries are pressure tests of this claim. When a port loses, first ask
whether the syntax and proofs were enough — not whether the peephole needs
another shape.

## How to write so the compiler can work

Every construct is a proof, or it is a boundary.

| Write this | Compiler may |
|---|---|
| `struct` / `class` with closed fields | scalar-replace `LocalOnly`; positional arrays; mangle owned names |
| `class` methods (`this` already exists) | static devirtualize; dissolve if identity unobserved |
| `enum` + `match` | integer discriminant, exhaustive DCE of arms |
| `int` vs `number` | proven `\|0` elision vs binary64 |
| `pure` / inferred effects | delete unused calls |
| static `import` | cross-file SSA; no ESM wrappers in the artifact |
| `extern` / `extern class` only at the host | exact ABI; everything else dissolves |
| `object O { … }` for a closed singleton API | ABI keys; private bodies nest/mangle |

Proof-driven optimization is not permission to ignore explicit target intent.
The IR separates source-authored operations from compiler-generated
normalization. A live source `x | 0` must remain a JavaScript `|0`; a generated
`|0` needed only to implement `int` may disappear after range proof. Future
exact-JS requests are narrow typed intrinsics with specified cross-target
behavior, not raw JavaScript strings or a file-wide optimizer-off pragma. See
[contracts before objectives](../decisions/contracts-before-objectives.md).

| Avoid this on hot internals | Why it taxes *T* |
|---|---|
| `JsValue` bags + `setProp`/`getProp` | string keys; no field deletion; every `o[k]` is a getter/proxy hook unless `assume_pure_property_reads` |
| `JS.method*` / `defineProperty` constructor tables | identity reconstructed in user space; peephole then tries to fuse `class` |
| `record{}` when the JS contract is ordinary `{}` | `Record<T>` materializes null-prototype; jQuery observes `Object.prototype` |
| `createEmptyObject()` as the default object | host trampoline; optimizer can lower some calls to `{}`, not a type |
| `int kind` ladders instead of `enum` | closed `match` is already in the language (Zod/Acorn ports still use ints) |
| `JsValue` when an owned typed optional bag is sufficient | dynamic keys block field/layout proofs and preserve observable coercion/property behavior |

Changing an **internal** API to the LilScript form is the port. Cargo-culting
Sizzle/jQuery/MobX object shapes is how you donate the advantage back to Terser.

Public names are config (`[mangle].exports`, `public_aggregate_abi`), not an
excuse to keep internals as string maps.

The planned reusable-library architecture is still closed-world optimization behind
an open boundary. It preserves a generated manifest of exported names,
callable identity/arity/constructibility, public field names or opaque-handle
ABI, descriptors, and host names. Internal names, owned fields, closure capture
slots, layouts, and call boundaries remain eligible for optimization. Closed
application output uses the same source API with no unknown external consumer,
so more of that manifest becomes internal.

## Typed rewrite is not a theorem of smaller *T*

A proof **authorizes** representations. It does not pick the winner.

md-01 showed that source rewrites are not a theorem: a micromark state-bag to
captured-local rewrite measured −114 raw / −2 Brotli but broke a streaming test,
and other typed-bag rewrites lost Brotli versus the prior `JsValue` source.
Those experiments changed representation and semantics; they are **not** a
valid scalar-replacement on/off ablation. The compiler now has a scored
`keep-object` IR clone when `joint-representation-search` is admitted, so future
measurements can isolate the representation question directly
([registry](../compilation/decision-registry.md#aggregates-class-struct-object-record)).

The design: prove the value is plain data / local / identity-free, **then**
score dissolve vs array vs named object vs (when identity is observed) named
`class`. Forcing dissolution because “structs are smaller” is the same class
of glue as forcing `class` because “ES class is smaller.”

## What the forks actually did

Inventory from in-tree ports and the board notes that measure out-of-tree
`@itslil` packages. Numbers are directional; cite the named evidence page
before quoting bytes.

### Typed rewrites — compressor sees proofs

| Port | Shape | Outcome |
|---|---|---|
| gl-matrix | `float[]` / `Float32Array` kernels, `int` loops | Brotli win on the complete ESM root |
| mitt | `struct Emitter` + `Map` | essentially tied; one `JsValue` cast at a union call |
| nanoid | typed `Crypto` / `Uint8Array` | win/tie on the browser entry |
| monaco piece-tree | `struct` / `class` / `enum NodeColor` | large Brotli win vs the fair JS extract |
| marked (`@itslil/marked`) | typed parser, **no** host file | Brotli win vs Oxc parse-only |
| solid core (lab) | typed signals | ~parity |
| `comparison/apps` | written as LilScript from scratch | Closure ADVANCED gate |

### JS-shaped ports — compressor is forbidden to invent proofs

| Port | Forced glue | Missing / unused language |
|---|---|---|
| jQuery | `js-host` `callN` / `setProp`; `jQuery["fn"]`; `JS.method*`; Deferred/jqXHR as `JsValue` facades | typed homogeneous ordinary dictionaries/spread; host-callable typed `this`/rest; wider sound array-ness; cannot replace observable ordinary objects with `Record` |
| monaco public API | `JS.object()` + `JS.method*` facade; `js-host.ts` for coalescing / `createElement` / string `+=` bugs | constructible class export; sound coalescing (**compiler**, not syntax) |
| Motion full DOM | option/keyframe `JsValue` bags; `asVisualElement` casts | structural/optional object types, overloads, getters, first-class tasks |
| MobX | `Proxy` / `Reflect` / `defineProperty`; `Atom["prototype"][…] = JS.method*` | Proxy stays host; constructible class; getters |
| Immer `produce` facade | trap tables | same; typed `ImmValue` COW is the LilScript path |
| markdown stack (`md-01`) | mechanical `JsValue` transliteration; every `o[k]` unstable | plain-data objects (`assume_pure_property_reads` is a flag, −6 359 Brotli, not a type); CommonMark constructs 2.8× source vs official JS |
| PostHog error-tracking | legacy constructable classes via `JS.method*` + `defineProperty` | migrate to landed constructor-value export and IR named `class` where API-equivalent |
| clsx | `JsValue` + `for-in` + `arguments` | legitimate hatch: the API **is** a dynamic walk. Rest `JsValue[]` would still be `JsValue` |
| solid web | `domSpread` / reconcile trampolines | host bags, not missing `struct` |

clsx is the control: a genuinely dynamic public contract should use `JsValue`
and may lose a few Brotli bytes. jQuery internals are not that contract.

Vendored unminified host JS (`parse5-host`, KaTeX HTML) is not a language hole
and not a compiler hole. It is a port that stopped being LilScript.

jQuery’s remaining compressed gap after search convergence is **IR control-flow
shape**, not identifier spelling. Normalized per 1 000 raw bytes vs
`jquery.min.js`: `if(` 1.85×, `else` 2.48×, `;` 2.21×, ternaries 0.89×.
Post-hoc hoisting, Yoda, commas, and forced `function`/`arrow` all **lost**
Brotli on the already-searched artifact. The emitter cannot invent a source
expression form the language never had; `local_phi_expression_regions` is a
codec-conditioned recovery, default **off** under Brotli.

## Existing surface ports underuse

Do not RFC these. They already exist.

| Surface | Contract | Typical underuse |
|---|---|---|
| `enum` + `match` | integer discriminant, exhaustive arms | Zod `int kind`, Acorn `TK_*` ladders |
| `object` singleton | ABI keys, private nestable bodies | rebuilt as `JS.object()` + `JS.method*` |
| `class` methods / `this` | typed receiver, static dispatch | use `export constructor` only when the constructor value is public |
| `pure` / inferred effects | unused calls removable | `JsValue` ops are effectful; purity cannot override that |
| positional `struct` | field indexes | `createEmptyObject()` for closed internal shapes |
| `Record<T>` | open **null-prototype** string keys | wrong for ordinary-`{}` dictionaries; right for maps that must not see `Object.prototype` |
| `JS.method0..3` / `JS.methodRest` | host-callable `this` + args as `JsValue` | intended hatch; emitter may fuse a private callback when identity does not escape. Not a typed method |

`export class` as **type-only** is also existing, and it is a language contract,
not an accident. [`docs/language-v0.1.md`](../../language-v0.1.md) § Modules:
struct and class names are compile-time type exports and do not produce
JavaScript bindings. Monaco, Motion, Zod, and Redux depend on that.
`ExportBinding::TypeOnly` in `src/lower.rs` implements it. Flipping `export
class` to emit a constructor would reintroduce `constructor` / `prototype` on
identity-free types the ports deleted on purpose.

## Remaining proof gaps and landed slices

Some former holes have a first implementation; others remain proposals. Every
new slice must land as syntax/semantics/analysis plus cases, or remain an
explicit unsafe ABI flag. None becomes a library-specific optimizer matcher.

| Proof the compiler needs | Why ports invent glue today | What must not happen |
|---|---|---|
| **Constructor value** distinct from type-only `export class` (landed) | `export constructor C [as PublicC];` publishes the constructor-value contract and drives named class emission. | Emitting `class` for identity-free cases; changing the type-only default |
| **Broader plain-data / no-hook proof** | Non-escaping compiler-owned `object{...}` allocations now forward statically own reads; dynamic/missing keys, writes, phis, returns, closures, globals, and host escapes cancel the proof. The markdown stack still needs broader ownership facts. | Silent `pure_getters`; treating an external object declaration as hook-free |
| **Typed ordinary-object dictionary** vs null-proto `Record<T>` | `object{...}` now provides explicit ordinary `%Object.prototype%` semantics as `JsValue`; a homogeneous typed dictionary and spread remain. | Inferring `{}` after record observation projection; treating ordinary objects as hook-free |
| **Destructuring/guarded `match`** | Expression `if(condition){left}else{right}` and enum/int/string/bool literal `match` are landed; destructuring and guards remain. | Reconstructing ternaries as an always-on Brotli prior; post-minify contraction of `if(` |
| **Host-callable typed method** (`this` + rest on a typed receiver, not `JsValue`) | `JS.method0..3` / `JS.methodRest` / `extern JsValue arguments` for public JS methods. Class `this` already exists for LilScript methods. | Treating JS `this` as a free optimization; fusing wrappers whose identity escapes |
| **Getters / setters as ABI** | `defineProperty` / Proxy traps when the published contract is an accessor | Peephole inventing accessors on dissolved fields |
| **Sound optional / structural bags** | Motion/Preact option objects become `JsValue` | Structural TS `any` |
| **Explicit target lowering contract** (first slice landed) | Live source `x \| 0` carries an IR obligation; globally unambiguous clone lineage and final-byte witnesses remain. | Treating every source spelling as frozen; raw JS text injection |
| **Application/library ABI contract** (partial) | Contract/objective and a source-derived manifest exist; world/format/roots and expected-vs-observed final ABI remain incomplete. | Disabling internal optimization in library mode; letting raw/gzip/Brotli alter the public API |

Proxy, Reflect, and `instanceof` constructor identity stay **host**. Faking them
in the type system would be Closure-style wishful renaming.

A plain-data proof must be structural and ownership-aware, not just a type name:
compiler-owned non-proxy allocation, no accessors, no prior untyped escape, and
either a proven-own key, controlled null prototype, or explicit pristine-
prototype assumption. An external ordinary `{}` value cannot make every missing
dynamic read pure because mutable `Object.prototype` may contain accessors;
validate/copy it at the boundary or retain conservative effects.
The implemented first slice follows that rule for non-escaping `object{...}`
allocations and proven-own constant keys; it does not consume
`assume_pure_property_reads`.

## Compiler bugs are not language holes

Monaco’s `js-host.ts` (`rbDeleteTree`, `emptyBuf`, `domCreateElement`) exists
because SSA coalescing and known-host lowering miscompiled large graphs
([ident](../migration/board/LEDGER.md) lane). That is 07.1 / identity work, not
a new keyword. Do not grow syntax to paper over an unsound coalescer.

Search ranking unresolved names ([ident-05](../migration/board/notes/ident-05.md))
is the same class: the selector admitted an invalid program. Widening search
or adding syntax does not fix it.

## Design rule

If a library cannot be expressed without `JsValue` on its **hot internals**,
either:

1. the public contract really is dynamic (clsx) — keep the hatch, measure, do
   not invent types; or
2. the port is still JavaScript — rewrite representation (`struct`/`class`/`enum`)
   until the compiler has proofs, then **search** dissolve vs keep; or
3. the language is missing a proof — use the reusable-proof migration, cases first, no peephole
   special case.

A fold that only pays on one port is glue. A syntax that makes every port able
to state a fact Terser must guess is the language. A search that scores the
legal spellings under the configured *T* is the compiler.
