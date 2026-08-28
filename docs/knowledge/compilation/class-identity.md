# Class identity vs instance lowering

Parent: [Compilation](README.md). Instance layouts:
[aggregate lowering](aggregate-lowering.md). Search:
[candidate search](candidate-search.md). ABI:
[JavaScript shape](../config/javascript-shape-abi.md). Language:
[packages / exports](../language/packages-exports-abi.md),
[compressor surface](../language/compressor-surface.md).

LilScript `class` is a nominal typed aggregate. The production IR backend
(`src/codegen_ir_js.rs`) therefore dissolves it: LocalOnly scalars, positional
arrays, or named objects, plus free functions `Foo$init` / `Foo$method`. That
is the compression win on jQuery internals, Motion cells, Monaco handles, and
the canonical `aggregates/class-*` cases. It is also why a published ES
constructor cannot be that lowering.

**Shipped vs planned (2026-08-28).** Dissolved instances are the configured
incumbent and are **not** competed against ES `class` for identity-free types.
When `AggregateLayout.identity_observed` is set, IR emit
(`src/codegen_ir_js.rs`) produces a named `class` plus `new Name(…)`, and skips
`$init` / `$method` free functions. `export constructor C [as PublicC];` sets
that mark and retains the constructor and public methods. Peephole fusion of prototype
tables remains for port `defineProperty` tables. Joint array-vs-object search
is a separate flag, omitted from root `lilscript.toml`, admitted by size-first
library configs. See
[decision registry](decision-registry.md#aggregates-class-struct-object-record).

`export class` stays **type-only**. It creates no JS constructor binding
(`ExportBinding::TypeOnly` in `src/lower.rs`). Do not change that default.
Monaco, Motion, Zod, and Redux export classes as instance types. Emitting
`class TextPos { … }` for those would reintroduce `constructor` / `prototype`
tokens the ports deleted on purpose.

The explicit constructor-value form requires a non-`object`, non-extern class.
A zero-arity constructor is synthesized only for a published base class that
omits `init`. Internal base chains are preserved and emitted base-first with
`extends` and `super`; a published derived class must state `init` and
`super(...)` explicitly. LilScript default field values emit as ES2022 public
class fields so initialization occurs before base constructor code and after
`super` for derived instances; lower syntax targets fail closed.

## Three representations

These are not a single `aggregate_layout` knob. Array-vs-named-object is
instance backing. ES class is constructor identity plus a method table on
`prototype`.

| Representation | Legal when | Default | Codec role |
|---|---|---|---|
| Dissolved / positional / named object | Constructor identity is unobserved | **Configured incumbent** | Almost always the Brotli/gzip/raw win |
| Function + `defineProperty` table | Constructor identity is observed | Port workaround only | Legal, large, fusion-fragile |
| Named ES `class` | Constructor identity is observed | IR emit when `identity_observed`; otherwise peephole fusion of tables | The compact legal spelling |

Identity observations (any one forces the constructor to stay a JS constructor):

- The constructor is a runtime ESM export (value, not type)
- `new C` / `C()` / `instanceof C` from untyped JS or `JsValue`
- `C.name`, `C.length`, `C.prototype`, or prototype method arity/enumerability
  is part of the published contract

`function_spelling` is the existing function-side analogue. There is no class
constructor ABI flag today. Do not add a TOML shape promise unless a library
publishes constructible class constructors. Carry the identity proof as
pre-search provenance, the same way `ordinary_record_literals` must not be
inferred from already-lowered IR.

## What error-tracking proved

`@itslil/posthog-js/error-tracking` exports 11 constructable classes. The
compat suite pins `constructor.name`, `constructor.length`, prototype names
(including `_`-prefixed methods), `{writable:false}` on `.prototype`,
`Object.keys(proto) === []`, method `.length` (defaults subtracted), and
`TypeError` without `new`. Official TypeScript is `export class ErrorCoercer`.
Oxc emits `Oe=class{match(e){…}coerce(e,t){…}}`.

The port cannot use LilScript `export class` as a value, so it rebuilds ES
classes with `JS.method*` + `Object.defineProperty` (`error-coercer-api.lil`,
`error-core.lil`). That is why the row is 28.5% larger raw and 18.7% larger
Brotli than Oxc, **despite** official `@/utils` barrel junk surviving in the
Oxc artifact.

Surveys, autocapture, and replay win because they export functions. Object
lowering is still the right default. This pack loses because it pays for
identity in user space.

A second, independent cut: production `parsed-peephole` already has
`fold_constructor_prototype_tables_to_classes`
(`src/js_peephole/folds/classes.rs`). It tries to turn those tables into
`class`. It currently:

1. Parses only `function`, not `async function`
  (`parse_function_expression` in `src/js_peephole/scope.rs`)
2. Emits methods without an `async` prefix (`emit_class`)
3. Uses the assignment form `C=class{…}` (anonymous → `C.name === ""`)
4. Then the port re-wraps with `defineProperty` for name / prototype
5. Can leave `await` in a non-async body, `async async function`, or
   `let t=e.prototype;;`

That is why error-tracking froze on `cost_model = raw` and
`candidate_search = off`. Search did not “prefer a worse Brotli shape.” It
rewrote identity tables into invalid JS. OTLP’s raw freeze is a different
bug (undefined binding after object-graph rewrite) and is not this page.

## Compact legal emit

Oxc’s anonymous `C=class{…}` fails this pack’s `constructor.name` check.
The identity-preserving compact form is a **named** class:

```js
export class ErrorCoercer {
  match(error) { … }
  coerce(error, context) { … }
}
```

or, when the binding must be short:

```js
e=class ErrorCoercer{match(t){…}coerce(t,n){…}};
export{e as ErrorCoercer};
```

That spelling gets name, arity, non-writable `.prototype`, non-enumerable
methods, default-parameter `.length`, `async` methods, and throw-without-`new`
from the language. No `defineProperty`. No `isPrototypeOf`. No
`JS.method1` adapter. Brotli then sees repeated `class` / `match(` /
`coerce(` the way it does on the Oxc artifact.

`constructor_initializer_fusion` and LocalOnly scalar replacement stay
**illegal** for an identity-observed constructor. They remain the default
for every other class.

## Search

Copy `constructor_initializer_fusion` / `inline_single_use_functions`:

1. Configured incumbent stays today’s object/array lowering.
2. A new internal `IrJsOptions` flag (working name `es_class_identity`)
   defaults **false** in `Default` and `js_options()`.
3. Identity-observed constructors **force** a constructable emit. The
   remaining choice is table vs named `class`, not object vs class.
4. `extend_javascript_candidate_beam` may flip table vs named `class` for
   those constructors only. Rank with `javascript.cost_model`.
5. Do not fold this into `named_aggregate_fields`. Joint representation
   search continues to score `{x:0}` vs `[0]` for identity-free instances.
6. Do not reuse the peephole as the only implementation. Fix it so existing
   tables remain searchable, then emit named `class` from IR so fusion is a
   no-op on this surface.

Under `cost_model = brotli`, the named class is the expected winner for a
forest of small exported coercers. Under raw/gzip the same form is still
the one that must beat Oxc/Terser: the 4 KiB raw gap is the table, not the
codec.

## Migration

### 0. Unblock Brotli search — **landed 2026-08-25**

Recorded in [ident-07](../migration/board/notes/ident-07.md) and
[search-03](../migration/board/notes/search-03.md). Compiler output for the pack
is now raw 15,332 / gzip-9 5,659 / **Brotli-11 5,156** against Oxc's 14,662 /
5,700 / 5,224 — Brotli and gzip win, raw does not, and the LilScript artifact
keeps the eleven class names Oxc mangles away. Compat 5/5. What it took, beyond
the list below:

- The proposal budget, not the fold set, was the binding constraint: an 18 KiB
  module at level 15 with `candidate_search = "production"` gets 96 work units
  against ~38 beam families. `posthoglil/lilscript.identity.toml` lifts it.
- Deleting the emulation the class replaces: the `new.target` guard call, the
  guard's declarator, the `name`/`length`/`prototype` finisher, and the
  `(function(){var v;v=…;return v})()` husk left by inlining the factory.
- `fold_undefined_defaults_into_formals` never descended past a `function`
  head, so a class inside an inlined factory kept `t===void 0&&(t={})` and the
  wrong `.length`.

Still open on this surface: fusion is not all-or-nothing on a table, so a
method whose value is an adapter call stays an enumerable prototype
assignment while its siblings become non-enumerable class methods.

### 0 (original plan). Unblock Brotli search

No port rewrite. Make fusion a legal scored leaf.

- Parse `async function`; store `async` on `Method`; emit `async name()`.
- Refuse fusion that would leave `await` in a non-async body, emit
  `async async`, or emit an invalid class element.
- Assignment form must be `C=class Name{…}` when `Name` is observed, not
  `C=class{…}` plus `defineProperty`.
- Do not emit a dead `proto=C.prototype` unless a later read needs it.
- Keep `validate_class_body_members`.
- Re-enable `lilscript.toml` (`cost_model = brotli`,
  `candidate_search = production`) on error-tracking and measure with
  `lilscript-codec`. Compat must stay green.

If this already beats Oxc Brotli, publish that artifact. The factories can
still die in phase 2.

### 1. IR named-class emit

Production path only (`codegen_ir_js.rs`). Legacy `codegen_js.rs::emit_class`
is not a size tactic.

- Prove constructor-value escape (runtime export, `JsValue`, host `new`).
- Emit named `class` with JS `this` methods, default parameters, `async`.
- Devirtualized internal calls may still be `Foo$method(instance, …)`.
- `export class` remains TypeOnly unless the constructor value itself is a
  runtime export.
- Add the beam family in `src/compiler.rs` next to joint representation
  search. Configured baseline stays in the set.

### 2. Port: delete the identity runtime

In posthoglil, after phase 1:

- Wire the 11 constructables as real classes (ErrorEvent methods must keep
  the official `_hasUsableMessage` / `_buildLocationStack` names).
- Delete `error-coercer-api.lil` factories and `runtimeConstructor` /
  `finishConstructor` / `requireNew` / `defineMethod`.
- Keep `createStackParser` as a rest function with `.length === 1`.
- Drop `lilscript.packs-safe.toml` for this pack.

### 3. Gate so it cannot regress

Add `comparison/cases/canonical/aggregates/exported-class-identity/`:

- JS side is `export class Coercer { match() {} coerce() {} }` (and one
  `async` method + one default parameter).
- LilScript side is the same surface, constructor exported as a value.
- Expect `le` on raw, gzip-9, and Brotli-11 against the best valid
  minifier. This is parity with native ES class, not a typed `lt` win.
- Existing `aggregates/class-scale` / `class-counter` stay `lt` and must
  keep dissolving. A change that emits ES class for those cases is a bug.

Pin `Function.name`, class names, arity, and constructibility in the case
oracle. Size is unread if identity drifts.

### 4. Out of scope

OTLP’s raw freeze, `@/utils` barrel junk on the official side, and
identifier-only aligned-mangling (−180 Brotli on the current 6,200 B
artifact) are separate. Do not post-minify the compiler output.
Do not turn object-lowering off globally to win one pack.
