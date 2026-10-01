# S4 public ABI contract and scope decision

S4 implements and checks the existing public contract. It does not introduce a
new runtime type witness, hidden public parameter, proxy identity or mutable
collection membrane. The previous phrase “complete wider public opaque
transport” had no finite ABI definition. This record replaces that phrase with
the concrete supported boundary below and records the separate design work.
It does not describe unsupported cases as implemented.

## Implemented boundary

| Crossing | Contract |
|---|---|
| Concrete value structs and instantiated nested structs | Public objects, declaration-order field snapshots and fresh public results; private storage stays private |
| Nullable structs, defaults and rest parameters | Shared value codec, selected absence pins and actual fresh rest arrays |
| Disjoint product/primitive unions | Select by disjoint runtime categories without probing object fields |
| Published constructors and prototype methods | Preserve nominal identity, constructibility, receiver, default/rest and reflected arity |
| Concrete struct-bearing callbacks | Value codecs when uses only invoke the callback; aliases and nested/returned closures may retain the private adapter |
| Ordinary struct-array parameters | A snapshot only under complete read-only/no-escape evidence and isolation from reentry, suspension and observable writes |
| Private erased products and collections | Complete typed interface; branches, recursive helpers, nested products, captures and owned concrete callbacks |
| Declared shapes and observed generic classes | Concrete field codecs and checked presence/default rules; reference identity remains unchanged |
| Host bindings | Bundled catalog, explicit JavaScript paths, native provider identities and source-owned capability diagnostics |

The public-interface proof runs before optional source transformations or
search, and target formation independently revalidates its immutable input.
Single-source and module-graph build/check routes share the proof and original
module/declaration span. It is mandatory ABI validation, not an effort heuristic
or a bypassable tactic. Private application exports are not public roots.

A read-only local use is insufficient to justify an array snapshot. For
`first = values[0].x; poke(); return first*10 + values[0].x`, a host callback can
replace the original element. A copied array incorrectly reads its old element
again. The compiler now refuses this crossing; it has not implemented mutable
host-array transport by fixing only that example. The shared operation-effect
owner supplies the isolation condition. An exact checked `JS.assume` identity
is inert; an actual host decoder can read getters and remains effectful.

## Separate ABI proposals, not migration completion claims

1. **Unrestricted public erased products.** A public generic entry has no complete
   set of concrete callers. An erased `T` alone cannot supply a nominal field
   schema or distinguish copying a struct value from retaining a reference.
   Private monomorphization does not solve an open JavaScript entry. A witness,
   an explicit bound or a different canonical public representation needs a
   specified calling convention first.
2. **Ambiguous product unions.** Two structs can both have the same public object
   shape. Reading fields to guess a variant can run getters and still cannot
   recover nominal identity. A declared discriminant or caller-supplied witness
   would be an ABI amendment. Supported disjoint unions need neither.
3. **Observable callable adapters.** Invocation-only wrappers are exact. Exposing
   arbitrary wrapper identity, properties, constructibility or frame reflection
   needs canonical bidirectional identities and a defined reflection contract.
   It must not silently narrow the existing Y3 contract.
4. **Aliased mutable product collections.** Copying the container loses host
   sharing. Eager conversion, copyback and an exposed proxy each change other
   observations. A public storage/element-view ABI must specify aliasing,
   snapshots, methods, descriptors, reentry and frozen inputs together before
   choosing a representation. This capability remains unimplemented.
5. **Erased public optional presence.** A generic null payload can mean a present
   `JsValue` or an absent optional field. Erasure supplies no rule for choosing
   omission. Existing explicit absence pins and concrete schemas remain exact;
   a generic presence witness is a new ABI decision.

These proposals are visible future language/ABI work, outside the current
migration's completion gates. This is a scope correction under the owner's
instruction to remove plan items that do not fit the objective, not a claim of
universal language or Closure parity. Inventing a public convention here would
change source/host behavior and could add compulsory wrappers to ordinary code
without a measured compression benefit. The migration continues with the
implemented checked contracts, conservative diagnostics, existing TOML controls
and measured representation work. Native completion remains N1/N2; source
contract adoption/default changes remain D3/V2; fleet qualification remains V1.
