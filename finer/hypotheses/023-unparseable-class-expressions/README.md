# 023 — The compiler was emitting unparseable JavaScript

**Status: BUG FOUND AND FIXED. Two ports could not build at all; the whole 16-port scoreboard is
regenerable again for the first time.**

## How it surfaced

[022](../022-harness-refresh/README.md) refreshed the five drifted `package.json` pins so
`run.mjs --measure` could see all sixteen ports. It immediately failed on something else:

```
Error: Build failed with 1 error:
../../../remarklil/dist/remark.esm.js:2:44708: ERROR: Expected ";" but found "Object"
```

Not a harness problem — **esbuild cannot parse the artifact LilScript produced.** Running
`npm run build` in `unifiedlil` fails with the same error, and its freshly written
`dist/unified.raw.js` — the compiler's own output, before any bundler — contains the defect at byte
10407. So this was live, not a stale artifact. `remarklil` I verified the other way round: its
committed artifacts are unparseable and nothing downstream can bundle them. Both build cleanly now.

## The defect

`src/js_peephole/folds/classes.rs`, in `emit_class`:

```rust
class.push('}');
if declaration_keyword.is_none() {
    class.push(';');
}
```

With a declaration keyword `emit_class` produces **two different shapes**:

| condition | emitted | needs `;` |
|---|---|---|
| `identity == name` | `class VFile{...}` — a *declaration* | no |
| `identity != name` | `var u=class VFile{...}` — a *statement* | **yes** |
| no keyword | `u=class VFile{...}` — an assignment | yes (was handled) |

The terminator was keyed off *the keyword* when it should key off *the shape*. The second row —
which is what you get whenever a constructor's binding differs from its observed `.name`, the
common minified case — was emitted bare. The class body then ran straight into whatever followed:

```js
var l=void 0,u=class VFile extends Object{constructor(e){/* ... */}}Object.defineProperty(u,"name",{configurable:!0,value:"VFile"});
//                                                                 ^ no separator: SyntaxError
```

`VFile` is why remark and unified were the two casualties: both wrap it, both hit the
binding≠name path. The `emit_proto_alias` branch a few lines below would have run a
`let p=u.prototype` into the class body the same way.

## The fix

Key the terminator off the shape that was actually emitted:

```rust
let emitted_declaration = declaration_keyword.is_some() && identity == name;
if !emitted_declaration {
    class.push(';');
}
```

Costs one byte on a shape that was previously invalid, so no valid output grows.

## Verification

- New test `class_expression_declarations_carry_a_terminator` pins all three shapes. Reverting the
  fix fails it with exactly `var u=class VFile{constructor(a){this.a=a}}`.
- `unifiedlil` and `remarklil` build again.
- Every artifact of all sixteen ports parses — **80/80**, was 76/80. The checker is
  `comparison/markdown-stack/.parsecheck.mjs`; running esbuild over every declared artifact is
  cheap and would have caught this years earlier than the scoreboard did.

## It never reached either repo's history

Worth checking rather than assuming: **all 8 artifacts committed to `remarklil` and `unifiedlil`
parse.** The unparseable files were working-copy rebuilds only, so nothing published carries the
defect and neither port's git history needs repair. Their working trees stay mid-migration and are
left alone.

## Scoreboard, all sixteen ports

Brotli delta, `lil-graph` vs `official-terser`, against the numbers published in `REPORT.md`:

| port | REPORT.md | fresh | change | verdict |
|---|---:|---:|---:|---|
| **rehype** | +9912 | **−1735** | **−11647** | **LOSS → WIN** |
| remark | +13329 | +10395 | −2934 | loss |
| **remark-gfm** | +379 | **−383** | **−762** | **LOSS → WIN** |
| katex | +6532 | +5800 | −732 | loss |
| remark-parse | +3738 | +3235 | −503 | loss |
| micromark | +4568 | +4154 | −414 | loss |
| mdast-util-from-markdown | +3573 | +3175 | −398 | loss |
| remark-math | +450 | +137 | −313 | loss |
| unified | +984 | +808 | −176 | loss |
| rehype-stringify | −745 | −794 | −49 | win |
| mdast-util-to-hast | −726 | −752 | −26 | win |
| remark-rehype | −671 | −687 | −16 | win |
| remark-breaks | −67 | −70 | −3 | win |
| hast-util-to-html | −1028 | −1014 | +14 | win |
| react-markdown | +16769 | +18552 | +1783 | loss |
| | | | **−16176** | **7 W / 8 L** |

Fourteen of fifteen improved, two flip to WIN, and `rehype` — the port
[006](../006-markdown-stack-loss-diagnosis/README.md) found the `minifyWhitespace` build bug in —
gains 11647 Brotli, the single largest honest movement recorded in this log.

## One row is excluded, deliberately

`rehype-katex` measures **−112118** and that number is meaningless. Its working tree is mid-refactor
to peer dependencies: the committed `scripts/build.mjs` has no `external` list and produces a
436511-byte bundle, while the working copy adds

```js
external: ["hast-util-from-html-isomorphic", "hast-util-to-text", "katex", "unist-util-visit-parents"]
```

and produces **2466 bytes that merely `import katex`**, with `dist/fontMetricsData.js`,
`dist/unicodeSymbols.js` and `dist/data-host.js` deleted. The harness dutifully compares that stub
against an official baseline that still bundles all of KaTeX. Counting it would book a 112 KB "win"
for shipping an import statement, so it is excluded from every total above.

**This is why `REPORT.md` is not regenerated in this hypothesis.** The harness can produce it now,
but one of its sixteen rows would enshrine a false win. The port's refactor should land or revert
first; then `--markdown comparison/markdown-stack/REPORT.md` writes a report that is true in all
sixteen rows.

## Open

- `react-markdown` is the only port that moved the wrong way (+1783). Also a `bundle`-mode port with
  a dirty tree, so it deserves the same scrutiny rehype-katex just got.
- The reflective-FFI finding in [021](../021-reflective-ffi-predicts-loss/README.md) still explains
  the micromark family, which is untouched by this fix.
