# LilScript language, version 1: the target contract

**Version 1, 2026-09-27.** This page is the language contract the compiler converges to. [language-v0.1.md](language-v0.1.md) is what the compiler accepts today. Every clause below carries a status:
- **in force**: the compiler implements it, and v0.1 says the same;
- **target (task)**: the plan task that implements it ([migration/plan.md](migration/plan.md); its state is in [migration/index.md](migration/index.md)); until it lands, v0.1's clause governs. When the task lands, the clause becomes *in force* and its v0.1 text is deleted. When every clause is in force, v0.1 is retired and this page is the only contract.

The 2026-09-29 replan preserves this language work. Legacy M-task labels map to current owners in [coverage.md](migration/coverage.md); use that map and the active checklist for implementation status. A planning change does not by itself implement a language clause.

**⚖ Yn** identifies an earlier owner decision, whose original wording is [archived](old-history/future-architecture.md#21-owner-decisions). Y1 and Y5 were approved; Y2/Y3/Y4 remain explicit in the active plan's [language compatibility section](migration/plan.md#10-language-compatibility-and-plan-maintenance). Changes to observable language semantics require an explicit language amendment. Rule ids R1–R18 remain stable.

v0.1 line references are to `git show d1d48c4c:docs/language-v0.1.md`. The [active plan](migration/plan.md) owns the compiler architecture; the [previous language rationale](old-history/future-architecture.md#12-the-language-designed-for-size) remains evidence. The rules come from the language review (`~/lilscript-work/out/design/review-language.md`, with probes and a fleet census in `review-language-probes/`), the Closure ADVANCED review's recovery-pass clauses, the runtime review's laws P1–P9, and three critiques of the first draft.

---

## 0. The principle

**Inside the sealed program every type is a guarantee, every host value is admitted once at a declared crossing, and every fact the compiler would otherwise infer or assume program-wide is stated by a type or a declaration.**

1. **Facts are stated, not recovered.** Purity, accessor-freedom, non-constructibility, closed variant sets and ABI names are types or declarations, never program-wide configuration.
2. **Boundaries establish; the inside relies.** No load, parameter or field read inside the program is re-normalized.
3. **JS-cheap semantics, one meaning.** Where the language chooses between a bare JavaScript operation and a normalized one, it picks the bare one and C emulates it; where the bare operation would let an optimizer change what the program observes (which absent spelling reaches a `print`, which enum representation reaches `JSON.stringify`), the language fixes one meaning or refuses the operation.
4. **Reflection is an operation.** Names, lengths, constructibility, key order and prototype identity are unobservable except at a declared boundary or on a nominal in the checker's reflected set.
5. **One meaning, capabilities per target.** Every type and operation declares its targets. The checker refuses non-portable use with a span.

Typed forms should remove avoidable representation and runtime costs, and declarations should expose guarantees the compiler can consume. These are engineering goals: static typing alone does not prove smaller compressed output or faster execution. The active plan verifies size under each objective and runtime on declared workloads. The older L13/P9/L14 labels identify the historical rationale.

**Evidence that the language is the primary lever.** On the five losing entries of 2026-09-27, about 1,040 of the roughly 1,800 verified recoverable Brotli bytes come from ceremony the language forces or fails to replace (review-language §2.3). The 27 ports hold 38,586 `JsValue` and 46,053 `JS.*` mentions, and use `ref` 0 times, `export constructor` 0 times and `match` 11 times. Typed ports run at or above upstream speed (markedlil 0.876 on its spec lane); untyped transliterations run slower (katexlil 1.10 in Node).

---

## 1. Identity and compilation model

**In force.** LilScript is an independent statically typed language; its identity, lexical grammar, statements and expressions are v0.1's (v0.1 "Identity and compilation model" through "Statements"), with these v1 changes elsewhere on this page. Nominal identity is per module: two modules' private classes, extern classes or enums of one name are two identities (M4.1, landed 2026-09-27).

---

## 2. Types are guarantees; host values enter at crossings (R1) ⚖ Y1

- **Clause.** Inside a program every value inhabits its static type: `int` is an int32, `string` a string, `T[]` a dense array of `T`, a class, shape or function value non-absent unless written `T?`. Host values enter only at declared **crossings**: export parameters; extern results, globals and extern-class fields; `import extern` bindings; trusted views (`as`, §3). A crossing is **trusted**: the host promises the type, and no code is emitted in production. An ill-typed value, or an accessor behind a key a shape declares as data (§6), at a crossing is a **precondition violation**: `checks = "development"` detects it at the crossing; production gives an unspecified but memory-safe result. Where upstream validates its input, the port declares `JsValue` and narrows it, so upstream's argument errors (D3.3) come from the port's own code, at the same point.
- **Replaces.** The unstated "types are hints" posture: load normalization (`src/program/javascript.rs:98-131`), producer-only int32 proofs (`src/program/javascript_int32.rs:1-10`), effect obligations (`src/program/effects.rs:17-27`), the `typed_arguments` assumption, D2 "Components".
- **JS.** Nothing at crossings in production. **Native.** The typed C ABI; tag checks in development builds.
- **Status.** Target (M10.12). The owner answered yes to Y1 on 2026-09-28: "runtime shouldn't have explicit type casting behaviours; only explicit typecast, type check gets compiled into instanceof". Until M10.12 lands, the value lattice's exact tier runs on literals, `const` and `define` alone (M6.4a).

---

## 3. The dynamic types (R12, R13)

### R12 `JsValue` and `unknown`

- **Clause.** `JsValue` is the dynamic type (`Type::Dynamic`), a JavaScript-only capability, with ordinary JavaScript syntax and JavaScript's meaning: member, index, call, `new`, `||`, `&&`, `??`, `?.`, `?.()`, `typeof`, `instanceof`, `in`, `delete`, spread, `for…of`, `for…in`, arithmetic, `==` (loose, as JavaScript defines it) and `===` (strict). Each is an observable evaluation point unless narrowed. `unknown` is dynamic too, but no operation other than `==`, `===`, `typeof`, `instanceof`, `is` and the ways out applies to it until it is narrowed. Leaving either type takes one of three spellings:
  - `v as T`: a trusted view, no code; a precondition checked by `checks = "development"`;
  - `v as? T`: a checked narrowing to `T?`;
  - `T(v)`: an explicit conversion that emits the coercion.
- **Replaces.** v0.1:76, :385-446 (the "narrow escape hatch", `JS.construct` with at most six arguments, dynamic equality as the M1.9 fix left it); 63 name-resolved `JS.*` builtins (`src/check.rs:43-118`).
- **JS.** Exactly the source syntax. **Native.** Refused by the checker with a span; a module that mentions `JsValue` or `unknown` is JavaScript-only.
- **Migration.** A mechanical script rewrites `JS.*` calls to syntax per port; source text keeps its meaning, so no `==` changes meaning.
- **Status.** Partly in force (batch S1, 2026-09-28): member, index, call, `new`, `||`, `&&`, `+`, `%`, the comparisons, `===`, `!==`, `typeof`, `in`, `instanceof`, `delete`, `v as T`, `string(v)`/`float(v)`, `undefined`, and `migration/js-builtin`'s fix-it for the `JS.*` spellings. Open: `?.`, `?.()`, `??`, the other arithmetic, truthiness, `as?`, `unknown`, and the refusal of the spellings (M4.2 with M10.2), in language slice 1.

### R13 Type tests on identity

- **Clause.** `v is C` and `v as? C` on classes whose identity is observed, extern classes, sealed and sum-type variants, and shapes with a declared discriminant.
- **Replaces.** v0.1:470-477 ("guards … limited to runtime categories"), `src/check.rs:9540`.
- **JS.** `instanceof` or a tag compare. **Native.** The existing immutable per-class destructor identity; no extra instance word.
- **Status.** Internal class identity tests and checked casts are implemented
  on JavaScript and native. JavaScript shapes declare an immutable literal
  `tag` for `is`/`as?`; enum/sum variants follow R8.

---

## 4. Absence (R2)

- **Clause.** `T?` holds a `T` or *absent*.
  - `T??` is `T?`. A type parameter instantiated with an optional type does not nest: `Map<K, V?>.get(k)` returns `V?`, and "missing" and "stored absent" are the same value (`has` tells them apart).
  - On JavaScript, absent is `null` or `undefined`; which one a value carries is unspecified inside the program. The optimizer may keep either producer.
  - **Operations that treat both alike** apply to `T?` directly: `== null`, `!= null`, `??`, `?.`, `?.()`, truthiness, narrowing, and equality `==` between two `T?` values (JavaScript's `==`, under which absent equals absent). `===` and `!==` are not defined on optional operands.
  - **Operations that could observe the spelling are refused on a `T?` operand without narrowing:** string conversion (templates, `+` with a string, `String(x)`, `print`), `typeof`, `JSON.stringify` of a value whose static type holds `T?` outside a shape field, `includes`/`indexOf`/`lastIndexOf` with an absent argument, `sort` of a `T?[]`, and `T?` as a `Set` element or `Map` key type. The author writes the narrowing (`x ?? "undefined"`, a `findIndex` with `== null`).
  - **Parameters with defaults** have type `T` inside. The default applies when the argument is omitted or absent. At an export crossing the boundary's rule is JavaScript's: omission or `undefined` takes the default, and `null` for a defaulted parameter is a precondition violation (§2).
  - **Fields.** For a nominal outside the reflected set (§7), an absent optional field is a slot holding either spelling. For a reflected nominal, an absent optional field is a **missing key**: construction omits it, and `JSON.stringify`, `Object.keys`, `for…in` and `in` see no key.
  - **Crossings pin one spelling.** A boundary type states `T | null`, `T | undefined` or an optional key; the crossing normalizes once, only where the producer's spelling is not already the pinned one. Converting `T?` to `JsValue` is such a crossing.
- **Replaces.** v0.1:77 (`T` or raw `null`), :145, :329-331 (`?? null`), :448-452.
- **JS.** Bare operations. **Native.** One absent tag; the refused operations are refused on both targets, so they agree.
- **Status.** Implemented under `[language] absence = "unified"`: shared
  checker/lint observation rules, collapsed optional types, internal absence
  equality/defaults, removal of internal read normalization, public optional-key
  omission and explicit null/undefined pins. Mutable collections retain their
  identity and normalize optional storage on writes; different storage pins
  require a copy. Defaults retain the same contract through functions, generic
  calls, constructors and `super`. `legacy` remains the migration default until
  qualified port adoption in D3/V2. Native absent-to-default transport has a
  source diagnostic pending N2. General erased product/union/callable transport
  remains R18; the absence selector does not claim that wider ABI work complete.

---

## 5. Declarations and definite assignment (R3)

- **Clause.** A local may be declared without an initializer; every read must be definitely assigned (flow-checked). A class or shape field must be assigned by `init` or the construction literal unless it declares its own initializer; there are no implicit type defaults. Reading `this` before every field is assigned is an error. Function declarations are not reassignable; `const` declares an immutable binding; "never reassigned" is a checker fact for every other binding.
- **Replaces.** v0.1:552-553 ("Every runtime variable declaration requires an initializer"), implicit field defaults.
- **JS.** `let x;` merged into its first assignment; construction initializes each slot once in the nominal's fixed order (future-architecture §10.2). **Native.** No zero-fill for scalars.
- **Migration.** Two batches (§14): a warning with a fix-it that writes today's implicit defaults explicitly, applied to every reference port as patches; then the refusal. Ports then delete dead placeholders.
- **Status.** Local definite assignment is in force. The class-field contract is
  implemented under `[language] field_initialization = "explicit"`, with one
  checker flow fact shared by the migration lint and all public checking/build
  routes. `legacy` remains the compatibility default while ports migrate; V2 owns
  qualified port adoption and the default transition. No effort level changes it.

---

## 6. Values, shapes, classes and variants

### R4 Structs are values; places update in place ⚖ Y2

- **Clause.** A `struct` is a value. A struct-typed place (a local, a field, an array element, a map value) is updated in place: `p.f = v`, `a[i].f = v` and `this.pos.x = v` mean `place = place with {f: v}`, and an update never copies. Assigning, passing, returning or storing a struct copies it **when it is stored into a second place while its source stays live**; when the source dies there, the store is a move and copies nothing (the uniqueness fact). `p with {f: v}` is an expression. A struct-typed class field may be stored flattened. `ref` is removed: a function that changed the caller's storage returns the new value instead (0 uses in 27 ports). Shared mutable state is a class or a shape.
- **Replaces.** D1's mutable references; v0.1:874-900 (`ref`); the implementation's tuple rebuild per field write (`src/program/javascript_structs.rs:1-3`).
- **JS.** Any layout the choice system picks; an update is a store into the place's storage; a copy is a fresh object or fresh scalars. **Native.** Value records; an update is a store.
- **Why this form.** It keeps L13 by construction: the cursor loop that runs 4.0× slower today updates a local, which is always in place; a copy happens only where an untyped program would need `{...p}` to keep the same meaning.
- **Status.** In-place updates and physical copy elision landed in S2. S4 adds `with`, which snapshots the original value before evaluating its overrides. `ref` remains supported; removing it still requires Y2.

### R5 Declared shapes for plain data

- **Clause.** A `shape` is a reference type for a plain object with a declared key set: `data` fields (reads are pure) and `accessor` fields; optional fields (§4); a construction literal `Token{type: t}`. A key's name is ABI only when the shape is reflected (§7) or crosses a boundary.
  - **Spread** `{...t, end: e}` copies the **declared** key set of `t`'s static type, with CreateDataProperty semantics, never the host object's other own keys.
  - **Intersections** `A & B` of shapes have the union of their key sets; a key declared in both must have the same type and kind, or the checker refuses the intersection.
  - A host object may be admitted as a shape at a crossing; it promises that the declared `data` keys are data properties (§2).
  - `extern class` stays for host-constructed objects with prototype methods. `Record<T>` stays for adversarial-key dictionaries (null prototype).
- **Replaces.** v0.1:137-143 (`object {}` typed `JsValue`, spread unsupported), :82 (`object O`) for data, the D2 refusal of structs nested in collections at boundaries (shapes are references); `assume_pure_property_reads`, `preserve_properties` and `public_aggregate_abi` for declared data.
- **JS.** Object literals and dotted reads; data reads may be forwarded and CSE'd. Spread lowers to JavaScript spread when the source is program-constructed, and to explicit keys when it may be host-admitted. **Native.** A record with a presence bitmask; host `accessor` fields are JavaScript-only.
- **Status.** Declared data/accessor shapes, contextual literals, checked
  intersections, declared-key spread, optional writes/defaults and literal tags
  are implemented on JavaScript. Concrete public product fields use the value
  ABI; development checks preserve accessor evaluation and validate recursive
  data. Native storage remains N2; erased public generic presence remains S4's
  R18 work. Micromark adoption belongs to D3, with qualification in V1.

### R8 Closed variant sets

- **Clause.** Enums with ABI values (`enum Kind: string { Text = "text" }`, `enum Code: int { A = 65 }`), flag sets, `Kind.from(abi) -> Kind?`, `k.abi`, `k.ordinal`. Sum types with payloads and exhaustive `match`. `sealed class` with `virtual` methods: per call site a static call, a tag switch, or a prototype method where identity is observed. Unsealed classes keep static dispatch and no overriding.
  - **Representation.** Every position that reflects, prints, stringifies or converts an enum value to `JsValue` uses its ABI value, converted at that operation; enum-typed fields of reflected nominals store the ABI value. An enum without a declared ABI refuses those operations except through `.ordinal`. Only unreflected positions have a representation the compiler chooses per objective, so output never differs by objective.
- **Replaces.** v0.1:101-123 (in particular :122 "string-valued external protocols require an explicit conversion"), :801-805.
- **JS.** Integers or ABI strings; per-variant layouts. **Native.** `int32_t` tags, C unions, a switch or a vtable.
- **Status.** ABI enums/flags, conversions, exhaustive matching and checked
  domains are implemented through shared evaluation and both targets. Declared
  ABI values are currently canonical storage, preserving mutable aliases;
  Q4 owns private representation alternatives with observation conversions.
  `[language] enum_abi = "explicit"` enforces the no-ABI observation refusal;
  `legacy` remains the migration default. Payload matches over disjoint class
  identities or declared shape tags and sealed virtual/override calls are
  implemented. Arm bindings have fresh lexical scopes; dispatch evaluates its
  receiver and arguments once and uses the selected body's defaults. Erased
  generic dispatch and directly suspending payload arms remain S4 work, with
  source diagnostics. Qualified default adoption belongs to D3/V2.

---

## 7. Functions, receivers and reflection

### R6 Reflection is an operation ⚖ Y3 (constructibility)

- **Clause.** Function `name` and `length`, constructibility, property names, key order and prototype identity are unobservable unless a declared boundary publishes them or the nominal is in the **reflected set**. The checker computes the reflected set once for the whole program, after generic instantiation, as a closure:
  - `JSON.stringify`, `Object.keys`, `for…in`, `in`, conversion to `JsValue` (written explicitly, `v as JsValue`) and passing to an extern parameter mark the operand's nominal reflected;
  - a generic operand marks every instantiation that reaches it;
  - reflection is transitive over field, element and payload types;
  - values that leave through `throw`, a promise rejection or an argument to a host callback cross a boundary and are reflected;
  - every class with a host base (`extends Error`, `extends HTMLElement`) is reflected, because host methods read its fields.
  LilScript function values are never constructible. An export keeps `name` and `length`; a constructible export is declared (`export function`, `export constructor`).
- **Replaces.** v0.1:410-419 (every adapter output constructible and named), D2 "Reflection"; `keep_function_names`, `keep_published_function_names`, `assume_unconstructed_callbacks`.
- **JS.** Arrows where the body reads no `this`, `arguments`, `super` or `new.target`; named functions only where published. Property renaming never uses a name that an extern declaration or the platform catalog declares on a host prototype chain. **Native.** Names do not exist; exported symbols are declared.
- **Status.** Target (M10.14). The reflected set itself needs no ruling; the constructibility of exported plain functions waits for Y3.

### R7 Receivers and variadics are part of function types

- **Clause.** Function types may state a receiver, `fn(this: T, A) -> R`; literals and shapes may hold methods. Parameters may be variadic, `T... rest`, excluded from `length`. Calls may spread, `f(...xs)`.
- **Replaces.** v0.1:399-425 (13 adapter primitives, `extern JsValue this/arguments`).
- **JS.** `function(a){…this…}`, `(a, ...b) =>`, `f(...xs)`. **Native.** The receiver is the first parameter; a rest parameter is an array.
- **Status.** S4 implements typed receiver literals and function types, explicit `.call`, receiver-valued fields, real rest arrays for functions/methods/constructors, and typed array spread to rest parameters. Defaults and rest compose, with public `length` preserved on JavaScript and explicit array transport on native. Spreading into a fixed typed parameter list remains refused; legacy adapters retain their compatibility contract.

---

## 8. Numbers, collections, indexing and iteration

### R11 Integers, lengths and indexing

- **Clause.** `int` is wrapping int32 on every target (kept). `|0` appears only after an operation that can leave int32 without a range proof; loads never normalize. `length`, `size`, `indexOf` and `findIndex` are `int`: collections and strings hold at most 2^31 − 1 elements (a D3.10 resource bound). `a[i]` and `s.codeUnitAt(i) -> int` have an in-range precondition (native traps; `checks = "development"` throws; production does not check); `a.get(i) -> T?` is the checked read. `s.charCodeAt(i) -> number` keeps its JavaScript meaning (NaN out of range). Float `%` is added.
- **Replaces.** v0.1:517-535 (keeps wrapping, drops load normalization), :1032-1034 (`charCodeAt` returns `int`, 0 out of range), the unspecified out-of-range behaviour (JavaScript defaults `""`/`0`/`null` while the interpreter errors).
- **Migration.** `charCodeAt` changes type: the seven reference ports use it 47 times, many as `int c = s.charCodeAt(i)` (markedlil `src/str.lil:387`). The fix-it rewrites each to `s.charCodeAt(i) | 0`, which keeps today's meaning exactly, or to `s.codeUnitAt(i)` where the index is bounded by the string's length in the same loop head. Before index preconditions become production semantics, every port suite runs in the `checks = "development"` lane (§14).
- **Status.** Float `%`, bitwise ToInt32, `codeUnitAt`, development index checks
  and the `migration/char-code` fix-it are implemented. Production JavaScript
  index reads use the in-range precondition (K12). Checked array/binary `.get`
  reads are nullable; native direct array, UTF-16 string/code-unit and binary
  reads trap out of range. `[language] char_code_at = "number"` implements the
  number/NaN contract in checking, shared folding and both targets. `legacy`
  remains the migration default until D3/V2 qualify port/default adoption.

### R10 Typed intrinsics mean ECMAScript's originals

- **Clause.** Typed operations (`a.push`, `m.get`, `s.slice`, `new Regex("lit")`, `Math.imul`) are defined by their semantics and spelled with the original built-ins, which the contract assumes unmodified. Operations on `JsValue` have host semantics and are effectful.
- **Replaces.** `assume_pristine_builtins` for typed code; the regex-literal gate (v0.1:190-203).
- **Status.** In force (batches K1 and K7, 2026-09-28): a typed call's or intrinsic's `int` result carries no `|0` where the original returns an int32; a literal `new Regex` is the regex literal; lengths are bounded; compiler-written decoders and the store fold of a key a fresh literal already has assume nothing. `assume_pristine_builtins` now covers `JsValue` operations only.

### R14 Iteration is semantic; its lowering is a choice

- **Clause.** `for (x of a)` over `T[]` visits indices `0 ..` live length; JavaScript `for…of` and an index loop are both exact on dense arrays, and the choice system picks per objective within the runtime floor (an index loop stays on hot paths unless the calibration table says they are equivalent). Typed arrays: index loop only. `for (k, v of map)` and `for (x of set)` iterate in insertion order. `for…of` over dynamic iterables uses the iterator protocol (JavaScript-only).
- **Replaces.** v0.1:313-319 (a lowering promise in the semantics contract), :1062-1064 (maps and sets cannot be iterated).
- **Status.** Target (M10.16), in language slice 1.

---

## 9. Modules and host interop

### R16 Modules are sealed

- **Clause.** Every non-exported declaration has a complete, known use set. An `export` between internal modules is visibility, never ABI; the exports of every declared delivery entry are ABI (a package with subpath entries has several). `object` singletons are deleted (0 uses; they do not compile today): module namespaces and const records cover them.
- **Replaces.** v0.1:773-794.
- **Status.** Dual type/constructor imports and explicit re-exports are implemented.
  S4 removes `object` singleton syntax and its checker/AST/LSP machinery after a
  zero-use census of the maintained ports. Constructor publication now follows
  declared delivery roots and actual first-class/dynamic observations. Internal
  visibility alone permits class dissolution and private property naming; type
  interfaces and module initialization remain. D1–D3 own delivery production.

### R17 Host interop is declarative

- **Clause.** `extern` functions, globals and classes carry trusted types and a JavaScript host name or a C link name. `import extern` aliases are compiler-named. A platform catalog (ECMAScript and the DOM surface) declares `document`, `RegExp`, `console` and the rest once, with effect classes and target capabilities. A declaration may have a binding per export condition: a library's `browser` condition may decode entities through the DOM, as upstream micromark does, while its Node condition ships the table.
- **Replaces.** v0.1:366-383; `globalThis[...]` access; name-keyed host helpers (already dropped).
- **Status.** Target (M4.6, M10.17).

---

## 10. Effects and pins (R15) ⚖ Y4 (termination)

- **Clause.** `pure` means no observable effect **and termination**; effects are checked (M6.3, in force), termination is asserted (Y4). `pure extern` is trusted. A call whose result is unused is removed only when it is discardable: no observable effect, it cannot throw, and it terminates (a declared `pure` function may throw, so purity alone does not remove its calls). A `debug` effect class marks strippable logging and assertions (`console.debug`-style helpers, `invariant`, `warning`): they are stripped only by `strip_debug`, never because they are pure. `print` is a program effect and is never stripped. Author pins (`@pool`, `inline for`, `@choose(family = alt)`) fix a choice at a region and are honored by the choice system.
- **Replaces.** v0.1:909-914, :932-946, :321-327.
- **Status.** Partly in force (batch K6, 2026-09-28): `debug` before `void` or `extern void` declares the class; `strip_debug` drops the declaration's direct calls, keeping their arguments' evaluation, and the inliner keeps its body. `debugLog` stays strippable by its name until the ports declare it (`migration/debug-class`), at their release (M12.4). Open: termination (Y4), and pins with the choice system (M10.11's later half).

---

## 11. Const data and defines (R9)

- **Clause.** `const` declares deep-immutable data (arrays, records, shapes, struct arrays). An exported const object has exact keys; the host receives a boundary object. `const` functions are evaluated at compile time under a configured bound (D3.6), using only `exact` operations: IEEE-754 basic operations with correct rounding, integer operations and ECMAScript-specified algorithms (Number-to-String, string operations). An operation whose precision ECMAScript leaves to the implementation (`Math.sin`, `exp`, `log`, `pow` and the rest) is evaluated at run time; a `const` that needs one at compile time is refused with a span. `define` constants are bound by configuration. A function called with const data may be **specialized** on it at compile time (a validator compiled per static schema), under the same bound and the same exactness.
- **Replaces.** M9.8's site recognizer ("any root `let` whose value is a tree of constant data", `src/js/tables.rs:2-4`); runtime-built tables such as katex's `unicodeSymbols`; the case for runtime code generation (zod's `new Function`).
- **JS.** The encoding is the codec's choice, within the startup law (P6). **Native.** `static const` data.
- **Status.** Target (M10.3 const data and bounded evaluation; M10.11 defines; M10.19 specialization).

---

## 12. Generics (R18)

- **Clause.** A type parameter is not observable at run time. With §4's collapse of `T??` into `T?`, erasure (JavaScript) and monomorphization or boxing (C) are both exact, and the native plan chooses per instantiation.
- **Replaces.** v0.1:511-515 (its implementation note "Polymorphic functions are not inlined until…" leaves the contract).
- **Status.** Generic functions/methods, observed classes and fixed-schema callable
  crossings are implemented. Wider opaque generic product/union transport remains
  S4 work; native representation qualification remains N2.

---

## 13. Target capabilities

| Rule | Native meaning | Capability |
|---|---|---|
| R1 | Typed C ABI; development tag checks | portable (exports need the C library ABI, M11.8) |
| R2 | One absent tag; the same refusals | portable |
| R3 | No zero-fill for scalars | portable |
| R4 | Value records; updates are stores | portable |
| R5 | Record plus a presence bitmask | portable; host `accessor` fields JavaScript-only |
| R6 | Names absent; exported symbols declared | portable |
| R7 | Receiver as the first parameter; rest as an array | portable (a `JsValue` receiver is JavaScript-only) |
| R8 | int32 tags, unions, switch or vtable | portable |
| R9 | `static const` data | portable |
| R10 | Runtime implementations | portable (`Regex` after M11.6, on QuickJS's `libregexp`) |
| R11 | int32 wrap; bounds trap; `fmod` | portable |
| R12 | none | **JavaScript-only**, refused by the checker with a span |
| R13 | Existing per-class object witness | portable (extern classes JavaScript-only) |
| R14 | Index loops; insertion-ordered map iteration | portable (dynamic iterables JavaScript-only) |
| R15 | Target-neutral | portable |
| R16 | Entry exports only | portable |
| R17 | C link names per extern | per binding |
| R18 | Monomorphize or box | portable |

---

## 14. How each rule lands

**Two batches for every rule that refuses code the ports contain.** Batch 1: the rule is a warning with a checker fix-it; the fix-it is applied to every reference port as patches in `~/lilscript-work/portwork/` (port repositories change only at each port's release), and their suites are green. Batch 2: the refusal. A rule whose meaning change is silent (R11's index precondition, R1's trusted crossings) first runs every port suite in the `checks = "development"` lane, which throws where production would have gone unspecified.

**Before a rule lands, its sites are counted per port.** The census below is the upper bound known today; each rule's batch 1 re-counts at current port heads.

| Rule | Affected sites (census) | Port change |
|---|---|---|
| R1 | none in source; behavioural | development-check lane over every suite |
| R2 | 11,592 `undefined` ceremonies; every `??null` | fix-it deletes normalizations; refused spelling-observing uses get explicit narrowing |
| R3 | 8,609 placeholder-initialized declarations (upper bound) | fix-it writes implicit defaults explicitly |
| R4 | `ref`: 0 | none |
| R6 | exported plain functions | `export function` where a port relies on `new` |
| R7 | 3,513 adapter sites | rewrite script (`JS.methodN`, `JS.methodRest`, `JS.staticRest` → syntax) |
| R11 | `charCodeAt`: 47 in the seven reference ports | fix-it to `\| 0` or `codeUnitAt` |
| R12 | 46,053 `JS.*` mentions | rewrite script |
| R16 | `object` singletons: 0 | none |

Each rule lands with at least one port using it, its suites green, no Brotli loss, and its typed form at least as fast as the `JsValue` spelling (micro perf gate, P9). The census of `JsValue` and `JS.*` per reference port is reported at every batch.

**Order** (the plan's "Order of work" holds the batches): language slice 1 (R12, R7, R13, R14) needs no ruling; the core (R2, R3, R11, R10, R15's `debug` class, then R1 after Y1) comes before M6.4b and M6.7; data and variants (R5, R8, R9) before M9.8 grows; then R4, R6, R16, R18.

---

## 15. From the 2026-09-23 additions to these rules

The 2026-09-23 page called its language additions L1–L10, which collided with the architecture's laws; they were renamed once to R1–R18 and are frozen.

| 2026-09-23 addition | Rule | Plan task |
|---|---|---|
| L1 declared object shapes | R5 | M10.1 |
| L2 const data and tables | R9 | M10.3 |
| L3 receivers, constructibility, rest | R7, R6 | M10.4, M10.14 |
| L4 sealed virtuals, interfaces, sum types | R8 | M10.5 |
| L5 ABI-valued enums | R8 | M10.6 |
| L6 immutable value structs | R4 (values, in-place places, copy-on-store) | M10.18 |
| L7 JS-cheap absence, numbers and strings | R2, R11 | M10.9 |
| L8 first-class dynamic type | R12 (with `unknown`) | M4.2 + M10.2 |
| L9 casts and operators | R12 (`as`, `as?`), R13, R11 (float `%`) | M10.7 |
| L10 pins and defines | R15, R9 | M10.11 |
| (new) | R1 trusted crossings | M10.12 |
| (new) | R3 definite assignment | M10.13 |
| (new) | R6 reflection as an operation | M10.14 |
| (new) | R10 typed intrinsics | M10.15 |
| (new) | R14 iteration | M10.16 |
| (new) | R16 sealed modules, R17 host catalog, R18 generics | M10.10, M4.6, M10.17 |
| (new) | R9 specialization over const data | M10.19 |
