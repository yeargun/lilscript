# arch-07 — 07.7 language proofs and explicit lowering contracts

Parent: [ledger](../LEDGER.md). Status: landed. Plan:
[07.7](../../07-global-compressor.md#077--language-proofs-and-explicit-lowering-contracts).
Surface: [compressor surface](../../../language/compressor-surface.md).

## Question

Which facts can ports only state today as `assume_*`, `JS.method*` tables, or
`createEmptyObject()`, which source operations require exact target lowering,
and which application/library observations become syntax + cases?

## Current hypothesis

RFCs, not folds. The first narrow contract has landed: source `value | 0`
carries `NodeId`, source origin, and a lowering obligation through IR; generated
normalization stays objective-controlled. Until arch-05 supplies a target AST,
affected candidates conservatively skip parsed-text rewrites. Public
constructor-value integration now uses `export constructor C [as PublicC];`
over arch-04's IR representation.

| RFC | Unblocks |
|---|---|
| Constructor **value** export (keep type-only `export class`) | Connect public syntax to the named-class representation produced by 07.4 |
| Plain-data / no-hook object | deleting `assume_pure_property_reads` from port TOMLs |
| Ordinary-`{}` dictionary vs null-proto `Record<T>` | jQuery without `createEmptyObject()` |
| Expression-if / general `match` | statement vs `?:` as an IR family |
| Host-callable typed method; getters as ABI; optional bags | Motion / public JS methods |
| Source-authored i32 normalization | Live `x \| 0` survives every objective; generated normalization stays optimizable |
| Application/library ABI manifest | Stable reusable API with fully optimized/mangled internals |

The initial exact-intent syntax is deliberately only `value | 0` with a
literal-zero right operand after parentheses. `0 | value`, `value |= 0`, and a
constant alias remain ordinary bitwise operations until a typed target intrinsic
defines a broader contract.

Existing surface that is **not** an RFC: `enum`+`match`, `object`, class `this`,
`pure`, positional `struct`, `JS.method*` as the dynamic hatch.

## Constraints specific to this task

- Cases first. No peephole special case. No silent `pure_getters`.
- Do not flip `export class` to emit constructors.
- Proxy / Reflect / `instanceof` constructor identity stay host.
- No generic raw-JS strings or file-wide optimizer barrier. Exact target
  intrinsics must be typed and state their optimization envelope.
- `cost_model` and `priority` may not alter the public ABI or a source lowering
  obligation.
- Plain-data/no-hook requires owned non-proxy allocation, no accessors or
  untyped escape, and a proven-own key/null prototype/pristine-prototype
  precondition. An external `{}` is not hook-free by declaration alone.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | compressor-surface inventory | board notes md-01 / jquery-01 / class-identity | `assume_pure_property_reads` −6 359 Brotli (flag); typed bags lost Brotli; jquery post-hoc `?:` lost; `export class` TypeOnly | diag |
| 2026-08-28 | source `value \| 0` obligation | `cargo test --release --lib source_written_i32_normalization` | raw/gzip/Brotli and search off/always retain live `\|0`; dead function disappears | gate |
| 2026-08-28 | application/library contract split | `cargo test --release --lib compilation_contract` | 4 passed; objective separate; exported callable/constructor ABI manifest records names, arity, constructibility, and methods | gate |
| 2026-08-28 | constructor-value source ABI | `cargo test --release --lib explicit_constructor_export_preserves_named_class_identity`; `compilation_contract` | named class + aliased export; `.name`, `.length`, `new`, and public method pass; manifest kind is constructor | gate |
| 2026-08-28 | expression `if` | `cargo test --release --lib expression_if` | parser/typing/narrowing/laziness/source conditional phi/ternary emission pass | gate |
| 2026-08-28 | scalar literal `match` | `cargo test --release --lib scalar_literal_match`; `emits_scalar_match_as_a_conditional_region` | int/string/bool typing, exhaustiveness, laziness, and conditional-region emission pass | gate |
| 2026-08-28 | ordinary-object literal | `cargo test --release --lib ordinary_object_literal_preserves_javascript_prototype_semantics` | `%Object.prototype%`, own `__proto__`, literal DefineOwn behavior, and later inherited setter observation pass | gate |
| 2026-08-28 | ownership-derived no-hook slice | `cargo test --release --lib owned_plain_object_proof_forwards_only_proven_own_reads` | own constant read forwards; missing inherited getter and host-mutated escaped object remain observable | gate |

## Log

- 2026-08-28 — Scheduled as 07.7. Draft RFCs in parallel; do not add flags. — **OPEN**
- 2026-08-28 — Exact `value | 0`, initial ABI manifest, and explicit
  constructor-value export landed. Target AST, derived default constructors,
  richer guarded/destructuring match, typed object spread, and plain-data proof remain. — **OPEN**
- 2026-08-28 — Expression-if, scalar literal match, ordinary-object literals,
  ownership-derived own-read proof, constructor hierarchies/defaults, and ABI
  validation landed. Richer patterns and object spread are future extensions,
  not blockers for the 07.7 contract. — **LANDED**

## Next step

Migrate ports to constructor-value/object syntax before deleting their wrappers
or unsafe assumptions. Treat object spread and guarded/destructuring patterns as
separate future proposals.
Contract:
[size-first libraries](../../07-global-compressor.md#size-first-library-contract).
