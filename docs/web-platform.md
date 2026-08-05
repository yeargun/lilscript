# Web Platform Integration

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

## Binary Memory

`ArrayBuffer`, `SharedArrayBuffer`, and `Uint8Array` are optimizer-known core
types, not host wrappers. JavaScript emission uses the native ECMAScript built-ins;
C emission uses LilScript's byte-buffer and view representation.

```lilscript
SharedArrayBuffer storage = new SharedArrayBuffer(4096);
Uint8Array bytes = new Uint8Array(storage);
bytes[0] = 42;
Uint8Array header = bytes.subarray(0, 16);
```

The current contract supports fixed-length buffers, byte views, indexed byte
access, `slice`, and `subarray`. It does not yet include `Atomics`, `DataView`,
additional typed arrays, resizable `ArrayBuffer`, or growable `SharedArrayBuffer`.
Native `SharedArrayBuffer` preserves shared view identity within one process but
does not yet provide concurrent or atomic semantics.

ECMAScript permits a host to omit the `SharedArrayBuffer` constructor. Browsers
also gate cross-agent sharing on secure-context and cross-origin-isolation policy.
Applications that require it must deploy the corresponding COOP/COEP headers and
choose a browser target where the constructor is exposed. See the
[ECMAScript structured-data specification](https://tc39.es/ecma262/2025/multipage/structured-data.html)
and [MDN deployment guidance](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer).
