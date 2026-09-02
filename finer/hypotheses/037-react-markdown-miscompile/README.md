# 037 — react-markdownlil disables terminal search to work around a miscompile

**Status: FIXED, landed with this folder. The alias was dropped because two folds ask "is it read
*after*?" and a hoisted function 61 KB *before* the class reads it. react-markdownlil drops
`terminal_codec_probe_limit = 0` for −3351 Brotli (45258 → 41907), 120/120 tests, gap +14166 → +10815.**

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

## The mechanism, corrected

The first reading above — a false negative inside `same_scope_name_is_read_after`, a second hole of
the 033 kind — was wrong about *where* the hole is. The scanner is not confused by a scope it fails to
recognise; it is asked a question that hoisting makes meaningless. `Qa` is a function *declaration*
at byte 6231 of the artifact and `Ma=class VFile` is at byte 67117. Every read of `Xa` is textually
before the class, and every one of them executes after `Xa=Ma.prototype` because `Qa` is only ever
called later. "Read after" from the class forward sees nothing, correctly, and answers the wrong
question.

Two folds ask it, and both drop the assignment:

| fold | what it drops | why it thought it could |
|---|---|---|
| `fold_constructor_prototype_tables_to_classes` (`classes.rs`) | `alias=Name.prototype` after the class it emits | `identifier_is_read_after(alias, scan)` from the end of the table |
| `fold_unread_prototype_aliases` (`declarations.rs`) | any `alias=Name.prototype;` statement | `unread_pure_alias_is_live(after)` from the statement forward |

The candidate fix that was in the working tree when this session started — a textual scan of the
tokens *after* the class — is byte-identical to the broken build (141965 raw, `Xa` still assigned
nowhere), because it looked in the same direction. The suite passed with it; the suite had no
hoisted reader.

## The fix

`scope::binding_is_observed_outside_span` (new) answers the right question: is the binding observed
anywhere the rewrite does not absorb — before the span, inside any nested function, inside the span
from within a method body the class takes over. It is textual about order and exact about scope: an
occurrence under a function whose parameters or own `var`/`let`/`const`/`function`/`class` bind the
name, under a block that `let`s it, under a `catch` or `for` header that binds it, or inside an
expression arrow whose parameter it is, is that binding and is skipped. Where shadowing cannot be
proven it counts the occurrence, because a kept alias costs about 15 bytes and a dropped live one
costs the module. A first, scope-blind cut pinned every alias whose name a constructor parameter
happened to share and failed two existing tests; the shadow-aware one passes all of them.

The after-region stays with the existing forward scans, which know that a later same-scope write
kills the value; the new check covers what they cannot see and is `||`-ed with them. When the alias's
own *declaration* is what would go (`var alias=Name.prototype` as the declaration), every later
mention keeps it, reads and writes alike, since strict code throws on assignment to a binding that no
longer exists. The captured-method alias path in the same fold had the same blind spot and gets the
same check; a live read there leaves the capture in place rather than re-declaring `let alias;`,
which keeps the binding and loses the value.

Tests: the shape through the fold, through the whole peephole, and a ten-shape audit
(`hoisted_readers_before_a_module_assignment_keep_it_live`) that runs each program before and after
the peephole and compares node's output, covering the prototype alias, a declared alias, a read
through a call, a `Symbol` alias, dead pure assigns, copy declarators, standalone vars, single-use
temporaries, an arrow reader and a method body reading the alias. Only the two prototype-alias folds
were wrong; the others already survive a hoisted reader.

## Result

Same source, same configs minus the one line, `lilscript-codec`, level 13, working tree of the port.

| variant | artifact | raw | gzip9 | brotli11 | tests |
|---|---|---:|---:|---:|---|
| shipped (`terminal_codec_probe_limit = 0`) | `esm.js` | 150642 | | 45258 | 120/120 |
| line removed, HEAD compiler | `esm.js` | 142179 | | 41894 | module throws on import |
| line removed, fixed compiler | `raw.js` | 141991 | 48872 | 41835 | |
| **line removed, fixed compiler** | **`esm.js`** | **142205** | **49008** | **41907** | **120/120, types pass** |

The fixed artifact is 13 Brotli bigger than the broken one: that is `Xa=Ma.prototype;`, the
assignment the program needs. Both configs compile in 2m36s wall on the shared host. Suite: 1642
passed.

**Fleet A/B**, HEAD binary against the fixed one, every port rebuilt from the same tree, sizes from
the codec (`finer/out/fleet` snapshots; jquerylil and markedlil timed out at 45 min in both passes
and carry no number here; katexlil never recompiles — status.md lead 9):

| port | objective artifact | HEAD | fix | Δ Brotli | tests on the fix build |
|---|---|---:|---:|---:|---|
| mobxlil | `mobx.esm.js` | 15708 | 15578 | **−130** | 754 pass, 15 fail on both builds (`Set.prototype.union`: Node 20) |
| react-markdownlil | `react-markdown.esm.js` | 41894 (throws on import) | 41907 | +13 | 120/120 |
| unifiedlil | `unified.esm.js` | 4659 (throws on import) | 4666 | +7 | 222 pass, 2 fail in the new `vfile` lane (below) |
| remarklil | `remark.esm.js` | 39303 (fails `api`, `closed`) | 39333 | +30 | all pass |
| rehypelil | `rehype.esm.js` | 52363 (passes) | 52462 | +99 | 159/159 |
| 16 others | | | | 0 | byte-identical |

Three of the five moved ports were **broken on HEAD by this same bug**: unified, remark and
react-markdown all bundle the VFile whose prototype alias the hoisted accessor installer reads, and
their HEAD artifacts throw `Object.defineProperty called on non-object` on import. The bytes they
gain are the assignment plus the search re-deciding around it. rehype's +99 and mobx's −130 are the
search alone: a token-level diff shows different candidate choices (`g2!=null` against
`!(g2==null)`, `var` against `let`, `else{…}` against `else …`), not kept aliases; the HEAD binary
rebuilds unified byte-identically under a loaded host, so the movement is the fix's, not noise.
Net over objective artifacts: **+19 Brotli for four modules that load.**

unifiedlil's two remaining failures are in its new `vfile.esm.js` lane, which is not in its
committed dist: `file.message()` returns an `Error` that is not `instanceof` the exported
`VFileMessage`, and a deleted `name` does not fall back to the prototype's `""`. HEAD and the fix emit
that constructor identically (`x=(0,function(t,e,r){…return M(t,e,r)})`), so it is the port's
migration, recorded in status.md.

## Verdict

Confirmed and fixed. The 3364 the config was hiding is real (3351 after paying for the assignment),
the port takes it, and the compiler no longer needs a zero budget to be correct on it. Settled for
status.md: **a forward scan from an assignment is not a liveness check when the reader can be
hoisted**; the two prototype-alias folds now ask the whole-program question, and the audit test is
where the next elimination fold gets its hoisted reader.

react-markdownlil's tree carries a large uncommitted source-graph migration (45 files) that predates
this work; the one-line config change and the rebuilt `dist/` sit on top of it uncommitted, for the
port's owner to land with the migration. The fleet number for the port is a working-tree number
until then.

## Next

Land the port-side halves: react-markdownlil's config line and rebuilt dist, unifiedlil's and
remarklil's rebuilt dists, on top of their owners' migrations. Then status.md's ranked leads.
