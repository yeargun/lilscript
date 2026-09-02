# 046 — katexlil is untyped so closed equals open

**Status: CONFIRMED — the closed-world build (`extern_fields = false`, otherwise the open config) is
byte-identical to the open build (230882 raw / 57087 Brotli, both worlds), because the port declares
no field the compiler owns: every object is a `JsValue` bag and every class is a hand-written
`prototype` table. The +5800 is the port's shape, not the compiler's, and the site's "closed" lane was
the same program at level 8. One shape fix landed (the pre-expanded unicode table → the upstream
generator loop, −943 Brotli on the shipped ESM); the rest is a port migration. Opened and closed 2026-09-02.**
Lane: port. Objective: brotli. Ports: katexlil. Opened: 2026-09-02.

## Prior art

- **What the official lane can and cannot do.** Terser with `mangle: true` renames identifiers and
  keeps every property name (`propmangle.js` is off unless `mangle.properties`); the site's baseline
  is exactly that (`scripts/lib/official.mjs`, `terserOptions`). So in open world both lanes carry
  the same ~400 property names, and the fight is emitted volume around them. Closure ADVANCED renames
  properties by default (`RenameProperties`, not vendored — verify), which is what our closed world is
  supposed to buy: the compiler renames the fields it owns. It owns none here.
- **Multi-use constant inlining, the cost rule.** Terser replaces a `var x = "lit"` reference by its
  value only when `replace_size <= name_length + overhead`, `overhead = (name_length + 2 +
  value_size) / (references − assignments)` with `unused` on (`TERSER/compress/inline.js:303-313`;
  the single-use path is `inline_into_symbolref`, `:191-260`, and `fixed`/`escaped` bookkeeping is
  `reduce-vars.js:195-243`). A 6-byte `"math"` never replaces a 2-byte name used 900 times. Oxc's
  `inline_identifier_reference` (`OXC/peephole/inline.rs:104-153`) inlines constant values without a
  size rule, but only for `const` bindings whose value is a literal it deems cheap. Ours,
  `propagate_single_assignment_globals` (`LS/optimizer.rs:3345-3410`), replaces every load of a
  single-store non-exported global with its constant, no cost check, before the search sees it. On
  katexlil that is 1960 sites and −8207 raw against Terser's shape — and **+129 Brotli** the other
  way (Method 3): under this objective the compiler's choice is right and Terser's is wrong, so the
  raw rule is not adopted. Row added to refs/competitor-techniques.md §D.
- **Generated tables.** Upstream `src/unicodeSymbols.js` is a build-time generator that esbuild
  bundles as the loop (479 raw / 301 Brotli after Terser); the port had committed its *output*, a
  4179-byte literal (1117 Brotli). The same class as 006's unminified `vfile`: the artifact carries
  what the upstream toolchain computes at load.
- **Source maps as the instrument.** Terser composes an input map (`sourceMap.content`), esbuild
  emits one per bundle; the compiler on `feature/source-maps` emits Source Map v3 in `hidden` mode
  with byte-identical JavaScript (`docs/source-maps.md`). `scripts/attribute-map.mjs` in the port
  charges every token of both artifacts to a module and measures each module's marginal Brotli
  with `lilscript-codec` (the artifact minus that module's tokens).

## Claim

The port's number is a source-shape number (objective.md §6 step 3). Confirms: a closed-world compile
that differs from the open one only by `extern_fields = false` yields the same bytes (nothing to
rename), and the per-module map shows the loss in the port-only glue and the hand-written class
modules rather than spread evenly. Falsifies: the closed build is smaller by ≥ 500 Brotli (the
compiler does own fields and the loss is elsewhere), or the attribution spreads the loss evenly across
the `functions/*` modules (then it is emission, i.e. the compiler).

## Read

- `finer/objective.md` §5–6, `finer/status.md` (katexlil rows, lead 9)
- `~/katexlil/src/host.lil` (the `JsValue` helper layer), `src/domTree.lil:140-240` (a class as a
  prototype table), `src/symbols.lil:39-60`, `scripts/build.mjs` (stitch and cache key)
- `LS/optimizer.rs:3345-3410`; `TERSER/compress/inline.js:290-320`

## May touch

- `~/katexlil/` (source, build, site, tests); `finer/out/046/`; this folder; refs row; status/log

## Method

1. **Closed = open.** Same binary (feature/source-maps 4e799a8), same source (katexlil 54daa8c),
   `lilscript.toml` against the same file with `extern_fields = false`, both on a pool worker; `cmp`.
   A third variant with `mangle.exports = true` (objective.md §4's closed world) for the app floor.
2. **Attribution.** `node scripts/build.mjs --compile --force --map` (hidden map), then
   `node scripts/attribute-map.mjs --json ...` (`--owner function`: a token belongs to the module of
   its innermost function, since the compiler's map sends a property name to the class that declares
   it).
3. **Is the string inlining a Brotli loss?** On the artifact, replace the 1960 inlined
   `"math"/"main"/"rel"/...` call arguments by 2-byte names declared once (Terser's shape); codec both.
4. **One shape fix, A/B on the pool.** `src/unicodeSymbols.lil` (the generator loop, port idioms)
   replaces the committed table; same binary, `cmp` of everything else; the port's Node tests,
   the official Jest suites, the Playwright browser test.

```sh
node finer/tools/workers.mjs sync --ports katexlil --compiler .claude/worktrees/agent-a5be67e76a2b44fdd/target/release/lilscript
ssh lilfarm@10.1.0.19 'cd ~/lil/katexlil && LILSCRIPT_COMPILER=~/lil/lilscript/target/release/lilscript node scripts/build.mjs --compile --force --map'
cd ~/katexlil && node scripts/attribute-map.mjs --json /tmp/attribution.json && node scripts/measure-site.mjs --spec --attribution /tmp/attribution.json
```

## Result

Compiler feature/source-maps 4e799a8 (main 20f4e09 plus the map branch); katexlil source 54daa8c
unless stated. Sizes `lilscript-codec`.

| variant | raw | gzip9 | brotli11 | notes |
|---|---:|---:|---:|---|
| open, compiler output (`katex.raw.js`) | 230882 | 68623 | 57087 | was 231519 / 69705 / 58342 with f504c93: −1255 from 041/044, never measured on this port until the cache key carried the binary |
| closed = open + `extern_fields=false` | 230882 | 68623 | 57087 | **byte-identical** (`cmp`) |
| closed + `mangle.exports=true` | — | — | — | refused: "generated JavaScript callable ABI mismatch" after 6 s (compiler defect, logged below) |
| open, the 1960 inlined strings as 2-byte names | 222675 | 68306 | 57216 | Terser's shape: −8207 raw, **+129 Brotli** |
| shipped ESM, committed (f504c93 build, level-8 closed) | 293173 / 305460 | 83555 / 89419 | 68937 / 73230 | open / "closed" |
| shipped ESM, this binary, committed source (build A) | 292536 | 82473 | 67762 | −1175 against the committed dist; `site/results.json` of build A in `finer/out/046/` |
| build B: A + `unicodeSymbols.lil` generator (shipped ESM) | 289019 | 80966 | 66819 | **−943**; compiler output 231459 / 68935 / 57314 (+227 for the loop, the 1117-Brotli table gone); closed still `cmp`-identical; port tests 17/17 |
| Terser of the published graph (site baseline) | 267050 | 76480 | 63044 | |
| Terser of the Flow sources via esbuild (strongest) | 263644 | 74591 | 61692 | `scripts/lib/official.mjs fromSource` |

Attribution (function ownership, compiler output vs the strongest lane; the font-metrics table is
outside both): the modules where we are bigger under Brotli are `entry` +358 (the port's public
glue: `Array.prototype.push.call` for one-element arrays, `!(x===void 0)&&!(x==null)` for one nullish
test), `symbols` +331 (not the inlining — Method 3), `Settings` +155, `buildHTML` +143,
`svgGeometry` +121, `Parser` +97, `wide-character` +96, `host` +86, `domTree` +63; 60 of 88 modules
are *smaller* than Terser's, most of `functions/*` by 50–170 each. Shape counts in the artifact:
230 `var x=void 0;` (every `JsValue x = undef(); x = …` pair), 118 `X.prototype.m=function`
assignments against 8 `class` keywords (Terser: 550 `const`, classes throughout), 23
`(0,function(){…arguments[i]…})` constructors, 121 `while` counters where Terser has 2, 86
`Array.prototype.push.call` where Terser has 0 — the `host.lil` helper layer verbatim.

Tests on build A: port Node tests 17/17, official Jest 1230/1230; browser parity and timing in the
port's `site/results.json`.

Defects found on the way, none fixed here:
- `mangle.exports = true` on this port: "callable ABI mismatch" (expected the five public functions,
  observed the mangled names) — the ABI check compares against the un-mangled manifest.
- `optimization_level = 0`: "unresolved generated export binding … `ka as buildMathML`" on the
  committed source too; level 8 and 13 compile.
- katexlil's build skipped the compiler whenever `dist/` was newer than `src/` — fixed in the port
  (the binary's mtime is in the key), which is why the −1255 above only appears now.
- The site's performance claim (0.32 ms vs 3.97 ms, "12× faster") did not reproduce: on the shared
  corpus, Node and Chromium both put the port within ±15% of upstream. Replaced by a Playwright-driven
  benchmark page and a generator script for every number on the page.

## Verdict

Confirmed. The closed contract has nothing to rename on this port, so closed world cannot pay; open
world loses on the glue and the hand-built classes, exactly where the `JsValue` layer is thickest, and
wins where the compiler sees plain functions. The lever is the port (objective.md §5): typed classes
for `domTree`/`mathMLTree`/`Settings`/`Options`/`Parser` and a `host.lil` that stops spelling
`push`/`newArray`/`isUndef` as helpers. The string-inlining raw excess is not a Brotli loss and the
compiler's rule stays. What status.md carries: katexlil's number is a source number; closed = open
until the port is typed; two compiler defects (exports ABI, level 0) with reproducers.

## Next

Migrate `domTree.lil` and its constructors in `buildCommon.lil` to `class` with typed fields (the
consumers read `.classes`, `.children`, `.height`, `.depth`, `.style` through `JsValue` today, so the
step is the module plus its readers), rebuild both worlds on the pool, and expect closed < open for
the first time; the open-world gain is the `(0,function(){arguments[i]})` and prototype-table
overhead, ~1 KB raw on this module alone.
