# 037 — react-markdownlil disables terminal search to work around a miscompile

**Status: CORRECTNESS BUG FOUND, REPRODUCER PINNED, NOT YET FIXED. It is worth 3364 Brotli and the
port cannot take that win until the bug is fixed.**

## What the config is hiding

`react-markdownlil/lilscript.toml` carries a line no other port has:

```toml
terminal_codec_probe_limit = 0
```

That reads as a compile-time concession on the fleet's largest artifact. It is not. Zero also trips
the entry guard in `apply_selected_canonical_peephole`, so the port ships **without the canonical
rewrite at all** — 726 unmerged `;var ` runs against micromarklil's 117.

Removing the line is worth a great deal:

| | raw | Brotli | `;var ` |
|---|---:|---:|---:|
| with `terminal_codec_probe_limit = 0` | 150642 | **45258** | 726 |
| without it | **142179** | **41894** | 198 |
| | −8463 | **−3364** | |

It also compiles in **1m54s**, so the line is not buying meaningful time either. react-markdownlil's
gap would go from +14166 to **+10802**.

## And it breaks the port

| config | tests |
|---|---|
| as shipped | **120 pass, 0 fail** |
| with the line removed | **module fails to load** — 8 tests ran, 5 failed |

```
TypeError: Object.defineProperty called on non-object
    at Qa (dist/react-markdown.esm.js:5:6256)
```

## The miscompile

`Qa` is the accessor installer, and it throws on its *first* statement, so the target is not an
object:

```js
function Qa(a,b){ Object.defineProperty(Xa,a,b); … }
```

`Xa` is the `VFile.prototype` alias, and the emitted artifact declares it and never assigns it:

```js
var Xa=void 0, Ma=class VFile extends Object{constructor(a){…}}
```

This is the same shape [023](../023-unparseable-class-expressions/README.md) fixed the terminator on
— the class rewrite with a prototype alias — and the same *family* as
[033](../033-member-bodies-are-scopes/README.md): a use that the rewrite fails to see.

`emit_class` only emits `alias=Name.prototype` when

```rust
let emit_proto_alias = proto_alias
    .is_some_and(|alias| identifier_is_read_after(&tokens, &matching_close, alias, scan));
```

`Xa` **is** read after — `Qa` reads it — so `identifier_is_read_after` has a false negative, the
assignment is dropped, and every later read gets `undefined`. It resolves through
`same_scope_name_is_read_after` in `scope.rs`, which is the same machinery whose member-body blind
spot 033 fixed; this is a second hole in it.

## Why this matters more than the bytes

The port is correct today only because someone set a budget to zero. Nothing records that the line is
load-bearing, and it looks exactly like a performance tweak — the next person to tune this port for
size will delete it, get a 3364-byte win, ship a module that throws on import, and have no way to
connect the two. That is now written down.

The general point is the same one [029](../029-specialisation-is-not-the-lever/README.md) and
[036](../036-the-fix-exists-and-starves/README.md) reached from other directions: **a config value
that silently disables a whole stage is indistinguishable from one that tunes it.** `0` here does not
mean "fewer probes", it means "skip the canonical rewrite entirely", and the entry guard cannot tell
a configured zero from an exhausted budget.

## Reproducer

```sh
cd react-markdownlil
sed -i '/^terminal_codec_probe_limit = 0$/d' lilscript.toml
node scripts/build.mjs --compile
node -e 'import("./dist/react-markdown.esm.js")'   # TypeError at Qa
git checkout lilscript.toml && node scripts/build.mjs --compile
```

The port has been restored to its passing configuration: 120/120, 45258 Brotli.
