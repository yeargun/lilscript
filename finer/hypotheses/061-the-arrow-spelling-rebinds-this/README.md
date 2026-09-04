# 061 — the arrow spelling rebinds `this`

**Status: SPLIT — the port was wrong and is fixed (`JS.method1`, one site), but
`function_spelling` is not only a spelling: it changes what an ambient `this`
read inside a closure means, and the syntax gate cannot see the difference.**
Lane: compiler. Objective: brotli. Ports: jquerylil. Opened: 2026-09-04.

## The observation

Shipped `@itslil/jquery` throws on `$(el).scrollTop()`, `scrollTop(1)` and
`scrollLeft(1)`:

```
TypeError: Cannot read properties of undefined (reading 'length')
```

Known since 042 and never diagnosed. The port's six compat tests do not call
them. It is not a search artefact: it reproduces at optimization level 13 with
`candidate_search = "off"` and `function_spelling = "arrow"`, in six seconds.

## Prior art

Terser is the only baseline that performs this rewrite, and it treats it as
dangerous in both directions:

- `lib/compress/index.js:4030` — `p(){return x}` → `p:()=>x` for an object
  method. Guarded by `!self.value.uses_arguments` **and**
  `!self.value.contains_this()`. On by default only because those guards hold.
- `lib/compress/index.js:3877` — a bare `function` expression → arrow. Guarded
  by `!self.uses_arguments` plus a full `walk` for `AST_This`, and it is still
  gated behind `unsafe_arrows`, which is **off by default**
  (`index.js:273`).

Oxc's minifier has no function→arrow conversion at 0.147.0. Closure ADVANCED
rewrites `this` only through explicit `goog.bind` devirtualization. Nobody
spells a `this`-reading function as an arrow, and the one tool that spells a
`this`-free one that way calls it unsafe.

## What the emitter actually does

Same source, one variable, level 13, search off:

```lil
extern JsValue this;
extern void collect(JsValue value);
void install(string name) {
  collect((JsValue arg) => { return this; });
}
install("m");
```

| `function_spelling` | emitted | meaning of `this` |
|---|---|---|
| `"function"` | `collect(function(a){return this})` | the **receiver** of the call |
| `"arrow"` | `collect(a=>this)` | the **lexical** `this` — `undefined` in an ESM module |

Both parse. The Oxc terminal-parser admission gate accepts both, because both
are valid JavaScript. Nothing downstream can tell them apart, so the candidate
search picks whichever is smaller and the program's meaning follows.

## Which one is right

The language already answers it, twice, in opposite directions:

- `codegen_ir_js.rs:7413` (`emits_ordinary_function_expression`) forces an
  ordinary function expression when the body reads `this` or `arguments` —
  **but only for `FunctionKind::Function`**, a declared function. A source
  closure is `FunctionKind::Closure` and falls through.
- `codegen_ir_js.rs` `nested_lexical_js_bindings_keep_the_callback_context`
  asserts the opposite for a closure: a nested `()=>this` must keep the
  *enclosing* `this`, not acquire a receiver.

So the closure rule is lexical, and the `"function"` spelling is the one that
breaks it. Extending the `FunctionKind::Function` guard to closures — the
obvious fix — was implemented, and it fails that test. Reverted.

`JS.methodN` / `JS.methodRest` (`docs/language-v0.1.md:393`) is how a port asks
for the receiver, and it means the same thing under every spelling. The port
already uses it 113 times.

## Result

A scan of every ambient `this` read in jquerylil against its nearest enclosing
function found **one** site that is not a declared function or a class `init`:
`src/offset.lil:250`, `installScrollMethod`. Rewritten to `JS.method1`, with the
stock HEAD compiler:

| build | scrollTop / scrollLeft | rest of the probe |
|---|---|---|
| shipped artifact | throws | `animate` also threw (`returnHr is not defined`) |
| level 13, `function` spelling, port fixed | pass | pass |
| level 13, `arrow` spelling, port fixed | pass | pass |

The port is now correct under both spellings, which is the property it should
have had.

## Verdict

Split. The port bug is fixed and shipped. The compiler hazard is real and
unfixed: a `[javascript]` knob documented as "an explicit JavaScript ABI and
spelling" (`docs/configuration.md:312`) silently changes the binding of an
ambient `this` read inside a closure, and the candidate search will take
whichever binding is smaller. Every port that reads `this` outside a declared
function is exposed; jquerylil was the only one that did, but nothing stops the
next one.

status.md should carry: **ambient `this` in a closure is lexical; only a
declared function is receiver-bound; `JS.methodN` is the portable way to ask
for a receiver.**

## Next

Make the `"function"` spelling preserve lexical `this` for a closure instead of
handing it a receiver — hoist the enclosing binding the way the arrow spelling
gets for free — so the two spellings agree and the search cannot choose between
two meanings. That is a compiler change under the fleet rule (objective.md §8)
and wants its own folder.
