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
runtime type checks. External global and member names are ABI names and stay
exact; `[host.javascript]` can select a declaration's global path but never
renames its members (the old `mangle.extern_fields` switch is retired). Internal values passed to or returned from a host
operation are marked as escaping so representation-changing optimizations remain
sound.

An `extern` means nothing by its name. The compiler keeps no table of host
helpers recognized by spelling: an extern called `createEmptyObject`,
`isWindowValue`, `mathMax` or `objectHasOwn` is an ordinary host call to
whatever the host binds under that name. (The deleted compiler route gave such
names built-in JavaScript bodies; that was dropped by design.) A program that
wants an operation the compiler understands writes it as one: a language
operation, a `JS.*` operation, or a host module delivered with the output
(`bundle.host_modules`). Plan task M10.2 replaces the `JS.*` operations with a
dynamic type and a typed host catalog in which each host operation has an
identity, a signature and an effect class.

Property reads and ordinary host calls are conservatively effectful because a Web
IDL getter or operation may throw, mutate host state, or run custom behavior. A
`pure` external method is a trusted host contract, and an unused call to it may
be removed (**until M7.2** the compiler keeps it):

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

## Bundled declaration modules

```lilscript
import {document} from "lil:dom";
import {console, performance, queueMicrotask} from "lil:ecmascript";
```

These are ordinary checked interfaces embedded in the compiler. Repeated
imports share one module and typed declaration identity. Unused imports add no
runtime code. The ECMAScript catalog covers console, timing/microtask functions,
and the `RegExp` host view; the DOM catalog covers events, nodes, elements,
text, document fragments and `document`. Host operations remain conservatively
effectful: importing a catalog grants no pristine-builtins assumption. Native
builds diagnose these JavaScript-only object contracts during checking.

Generic extern classes and methods erase type arguments, including inherited
fields and internal subclasses that call a declared host `init` through `super`.
The generic schema carries each checked substitution; host member names and
receiver identity remain intact. Host constructor signatures still serve
`super` calls; a direct `new` of an extern class remains refused.

## Scope

The hand-written declaration syntax is the implemented ABI foundation. LilScript
does not yet ship a complete generated browser declaration package. The intended
scalable source is the platform's [Web IDL](https://webidl.spec.whatwg.org/):
generated `.lil` modules can describe exposed interfaces while preserving the
same direct property and method lowering. Inheritance, overload sets, readonly
attributes, callbacks, dictionaries, and per-realm exposure still need explicit
language-model support before a complete Web IDL package can be claimed.

Host-object member access is JavaScript-target-only. The C and native targets
reject it with a source diagnostic because browser object identity and behavior do
not have a portable C ABI. Ordinary `extern` functions are the explicit route for
a user-defined C host ABI; explicit `[host.native]` mappings select callback ABI v1 provider symbols;
`[host] native_sources` supplies separately compiled C implementations. The
compiler delivers their exact header with C or executable output. Defaults,
implicit receivers and foreign globals still require explicit adapter functions.

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
