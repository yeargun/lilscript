# 028 — The harness was comparing our unminified bundle against their minified one

**Status: MEASUREMENT FAULT FOUND AND FIXED. 10634 Brotli bytes of reported loss were the harness,
not the compiler.**

## How it surfaced

[027](../027-tuning-is-exhausted/README.md) profiled `unified` and found the Lil artifact carried
**87 `var` declarations against Terser's 12** and **zero `if(`** — odd enough to look at the bytes.
They were not minified:

```js
function ae(e2, t2) {
  var u2, n2 = "", s2 = 0, i2 = -1, o2 = 0, r2 = 0;
  while (r2 <= e2.length) {
```

Newlines, indentation, spaces around operators, and `e2`/`t2` suffixes from esbuild's collision
renaming. The official side, in the same directory, is dense Terser output.

## The fault

`run.mjs` builds both lanes with the same options, then minifies **only the official one**:

```js
const lilBuild = await toolchain.build({...buildOptions, entryPoints: [lilEntry], write: lilBundled})
if (!lilBundled) copyFileSync(lilEntry, lilGraph)

const officialTerser = generatedPath(options, port, 'official-terser.js')
await minifyFile(toolchain, officialGraph, officialTerser)   // official only
```

For the thirteen ports where `lilBundled` is false this is harmless: the Lil lane is a straight copy
of the file the compiler wrote, which is already minified. But
`manifest.toolchain.graph.lilBundlePorts` is

```
['remark', 'unified', 'react-markdown']
```

and for those three the Lil lane is an **esbuild bundle with no minification at all**, compared
against a Terser-minified official graph. `unified`'s compiler output is 14580 bytes on 2 lines;
the harness measured 20869 bytes on 588 lines.

**Those three are exactly the three largest raw excesses in [025](../025-brotli-repetition-gap/README.md)**
— +53.3%, +53.7% and +85.3%, the top of the table. The statistic that raw volume predicts the losses
at r=+0.940 was measuring this bug at its own top end.

## The fix

Restore the density the compiler emitted, and nothing more:

```js
...(lilBundled ? {minifyWhitespace: true, minifyIdentifiers: true, minifySyntax: false} : {})
```

`minifySyntax` stays **off** deliberately. Whitespace and identifier renaming are what the bundler
destroyed and are ours to get back; syntax minification is optimisation the compiler did not do, and
the Lil lane must not borrow it. The conservative choice costs us real bytes — Terser on the same
input reaches 36606 on remark against our 38476 — and it is the only one that measures the compiler
rather than Terser.

## Result

| port | as reported | corrected | phantom |
|---|---:|---:|---:|
| react-markdown | +18552 | **+12923** | 5629 |
| remark | +10395 | **+5988** | 4407 |
| unified | +808 | **+264** | 544 |
| | | | **10634** |

No verdict flips — all three were losses and remain losses. What changes is that **41% of remark's
reported gap and 30% of react-markdown's were never real**, and `unified` is now 264 bytes from a win
rather than 808.

## What it invalidates

- The r=+0.940 raw-volume correlation in [025](../025-brotli-repetition-gap/README.md) is inflated by
  these three points and needs recomputing on corrected numbers. Its qualitative conclusion — we lose
  where we emit more — survives, because the nine other losses are untouched.
- 027's "unified emits 47% more functions, each 29% bigger" was measured on the unminified artifact
  and is wrong as stated.
- `fleet.mjs` was never affected: it measures each port's own `dist/`, so its numbers were right all
  along. Where the two disagreed, the fleet was correct — unified read +234 there against the
  harness's +808.
