# Web Platform Integration

Host ABI reasoning: [knowledge/language/boundaries-escape.md](knowledge/language/boundaries-escape.md). Progressive enhancement: [knowledge/delivery/progressive-enhancement.md](knowledge/delivery/progressive-enhancement.md).

LilScript is not a JavaScript syntax subset. It has its own static semantics and
implements only language features that can be checked and optimized consistently.
Browser APIs are a separate typed host ABI, declared with `extern class` and an
`extern` global.

```lilscript
extern class Element {
  string textContent;
  void setAttribute(string name, string value);
  void appendChild(Element child);
}

extern class Document {
  Element createElement(string tag);
  Element? querySelector(string selector);
}

extern Document document;

Element element = document.createElement("button");
element.textContent = "Run";
element.setAttribute("data-state", "ready");
```

For the JavaScript target, these operations lower directly:

```js
document.createElement("button")
element.textContent="Run"
element.setAttribute("data-state","ready")
```

The compiler emits no host wrappers, registries, proxies, reflection tables, or
runtime type checks. External global and member names are ABI names. Identifier,
property, and export mangling never changes them. Internal values passed to or
returned from a host operation are marked as escaping so representation-changing
optimizations remain sound.

Known pure host factories used by ports may also lower in the optimizer before
codegen: `createEmptyObject()` becomes a plain `{}` (distinct from null-proto
`record{}`), `createArray()` becomes `[]`, `callN(f, null, …)` becomes a direct
call, and rare DOM field getters expand to `.prop` when a mangled helper would
cost more than the property spelling. That is the same class of whole-program
knowledge Closure `ADVANCED` applies to externs — LilScript just starts from
typed `extern` contracts instead of JSDoc.

Property reads and ordinary host calls are conservatively effectful because a Web
IDL getter or operation may throw, mutate host state, or run custom behavior. A
`pure` external method is a trusted host contract and an unused call may be
removed:

```lilscript
extern class Clock {
  pure int cachedResolution();
  int now();
}
```

An external global binding is read-only in LilScript, while declared fields are
writable. Construction with `new` is forbidden for external classes; construction
must happen through the declared host API. External methods must be called through
their receiver so JavaScript's `this` binding cannot be lost accidentally;
function-valued external fields remain first-class callable values.

## Scope

The hand-written declaration syntax is the implemented ABI foundation. LilScript
does not yet ship a complete generated browser declaration package. The intended
scalable source is the platform's [Web IDL](https://webidl.spec.whatwg.org/):
generated `.lil` modules can describe exposed interfaces while preserving the
same direct property and method lowering. Inheritance, overload sets, readonly
attributes, callbacks, dictionaries, and per-realm exposure still need explicit
language-model support before a complete Web IDL package can be claimed.

Host-object member access is JavaScript-target-only. The C and native backends
reject it with a source diagnostic because browser object identity and behavior do
not have a portable C ABI. Ordinary `extern` functions remain the explicit route
for a user-defined C host ABI.

For APIs whose documented JavaScript boundary is intentionally dynamic,
`JsValue` preserves the raw host value rather than requiring an allocation-heavy
conversion tree. The available truthiness, category tests, dynamic indexing,
numeric `length`, and direct string-key `for-in` operations are specified in
[language-v0.1.md](language-v0.1.md). This remains a JavaScript-only ABI; native
targets reject it explicitly.

## Binary Memory

`ArrayBuffer`, `SharedArrayBuffer`, and the nine core typed arrays
(`Int8Array`, `Uint8Array`, `Uint8ClampedArray`, `Int16Array`, `Uint16Array`,
`Int32Array`, `Uint32Array`, `Float32Array`, `Float64Array`) are
optimizer-known core types, not host wrappers. JavaScript emission uses the
native ECMAScript built-ins; C emission uses LilScript's shared byte-buffer and
typed-array view representation.

```lilscript
SharedArrayBuffer storage = new SharedArrayBuffer(4096);
Uint8Array bytes = new Uint8Array(storage);
bytes[0] = 42;
Uint8Array header = bytes.subarray(0, 16);
Float32Array values = new Float32Array(16);
values[0] = 1.5;
Int32Array ints = new Int32Array(4);
Uint8ClampedArray clamped = new Uint8ClampedArray(2);
clamped[0] = 300;
```

The current contract supports fixed-length buffers, the nine typed-array views
above, indexed access, `slice`, and `subarray`. Integer views wrap; clamped
bytes saturate into `0..255`. It also supports shared indexing and `length`
for `float[]|Float32Array`, with tagged native dispatch for configurable
numeric kernels. It does not yet include `Atomics`, `DataView`,
`BigInt64Array`/`BigUint64Array` (no `BigInt` yet), resizable `ArrayBuffer`, or
growable `SharedArrayBuffer`.
Native `SharedArrayBuffer` preserves shared view identity within one process but
does not yet provide concurrent or atomic semantics.

ECMAScript permits a host to omit the `SharedArrayBuffer` constructor. Browsers
also gate cross-agent sharing on secure-context and cross-origin-isolation policy.
Applications that require it must deploy the corresponding COOP/COEP headers and
choose a browser target where the constructor is exposed. See the
[ECMAScript structured-data specification](https://tc39.es/ecma262/2025/multipage/structured-data.html)
and [MDN deployment guidance](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer).
