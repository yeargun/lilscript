# 039 — measurements

Run details and the full result table for [README.md](README.md).

### As run (2026-09-01, 21:50–22:40)

Terser 5.50.0, acorn 8.15.0, `lilscript-codec` (zlib 1.3.1 level 9; Brotli 1.1.0 q11 w22). The
copies are sha256 `24c92fb3…` (jquery, dist mtime 18:15 = the last fleet measure, built by the
working-tree compiler; `git status` shows it modified), `40118f0d…` (mobx), `ca1cdbbc…`
(micromark). Because the dist copy is not the committed artifact, the committed one
(`git show HEAD:dist/jquery.esm.js` in jquerylil, commit 30c000d of 2026-08-28) was measured
alongside it as `jquery.esm.committed.js`. No port was built, no compiler rebuilt.

- `finer/out/039/mangle.mjs` — Terser in five variants: `reprint` (mangle off), `mangle-locals`
  (`toplevel:false`), `mangle-all` (`toplevel:true`, the Method's step 1), `mangle-all-fixed-alphabet`
  (a `nth_identifier` with only `get`, so Terser skips `compute_char_frequency` and spells from the
  canonical alphabet — the spelling effect isolated *inside* Terser's own rename) and
  `mangle-all-keep-names` (`keep_fnames`+`keep_classnames`).
- `finer/out/039/relabel.mjs` — step 2. acorn parse, full scope resolution (module, function,
  block, catch, class, named-function-expression scopes; `var` hoisting), then a permutation of the
  identifier namespace: every declared name of length ≤ 2 maps to a new name of the same length,
  every free global, exported declaration and longer name maps to itself. A permutation of names
  preserves every binding relation by construction; the script still verifies, occurrence for
  occurrence, that the relabeled program binds to the same scope index, that the free-name set is
  unchanged, and that no mangled name is also free anywhere. Terser's frequency is reproduced
  literally (`lib/scope.js:976-1062`): every character of the program +1, every mangleable symbol
  occurrence −1, stable sort of the 54 leading characters then the digits, names numbered by
  `base54` with the first character varying fastest, handed out by descending occurrence count.
  `--alphabet=canonical|keyword|ours` give the same assignment under other alphabets.
- `finer/out/039/sizes.mjs` — the codec table.
- Gates: `node --check` on every relabeled file; `terser(relabeled)` is byte-identical to
  `terser(original)` for both jquery copies (a scope-preserving relabel must re-mangle to the same
  text, and does); the port's own `test/compat.test.mjs` cannot be pointed at a file (it resolves
  `dist/jquery.esm.js` from its own location), so it was copied unmodified into
  `finer/out/039/jqtest/` with the port's `node_modules` symlinked and `dist/` holding the artifact
  under test — 6/6 pass on the dist copy, the committed copy and both relabels. Plus an import and
  API smoke (`jqtest/smoke.mjs`, `probe.mjs`, `smoke2.mjs`).

## Full result table

All sizes from the codec; deltas are Brotli against the row's own "ours".

| variant | port | raw | gzip9 | brotli11 | delta brotli vs ours | tests |
|---|---|---:|---:|---:|---:|---|
| ours (dist copy, working tree) | jquery | 83778 | 32540 | 29270 | 0 | 6/6 (`animate` throws, below) |
| Terser reprint | jquery | 83685 | 32513 | 29295 | +25 | |
| Terser mangle locals only | jquery | 83375 | 31737 | 28505 | **−765** | |
| Terser mangle (step 1) | jquery | 83863 | 31803 | 28557 | **−713** | 6/6 |
| Terser mangle, fixed alphabet | jquery | 83863 | 31901 | 28595 | −675 | |
| Terser mangle, keep fn/class names | jquery | 83735 | 32073 | 28713 | −557 | |
| **relabeled, same lengths, Terser order** | jquery | 83778 | 32549 | 29243 | **−27** | 6/6 |
| relabeled, canonical alphabet | jquery | 83778 | 32639 | 29274 | +4 | |
| relabeled, keyword (ETAOIN) alphabet | jquery | 83778 | 32549 | 29252 | −18 | |
| relabeled, our own implied order | jquery | 83778 | 32562 | 29233 | −37 | |
| ours (committed, 30c000d) | jquery | 83044 | 31530 | 28225 | 0 | 6/6 (`animate` throws) |
| Terser reprint | jquery committed | 82973 | 31516 | 28267 | +42 | |
| Terser mangle locals only | jquery committed | 82973 | 31526 | 28274 | +49 | |
| Terser mangle | jquery committed | 83680 | 31721 | 28499 | **+274** | |
| Terser mangle, fixed alphabet | jquery committed | 83680 | 31809 | 28463 | +238 | |
| Terser mangle, keep fn/class names | jquery committed | 83068 | 31758 | 28502 | +277 | |
| relabeled, Terser order | jquery committed | 83044 | 31551 | 28257 | +32 | 6/6 |
| relabeled, canonical | jquery committed | 83044 | 31665 | 28367 | +142 | |
| ours (dist copy) | mobx | 56707 | 17379 | 15708 | 0 | |
| Terser mangle locals only | mobx | 56266 | 16676 | 15046 | −662 | |
| Terser mangle | mobx | 56235 | 16697 | 15056 | −652 | |
| Terser mangle, fixed alphabet | mobx | 56235 | 16761 | 15074 | −634 | |
| relabeled, Terser order | mobx | 56707 | 17369 | 15727 | +19 | `--check` |
| ours (dist copy) | micromark | 87117 | 30563 | 26097 | 0 | |
| Terser mangle locals only | micromark | 83999 | 29611 | 25312 | −785 | |
| Terser mangle | micromark | 84081 | 29732 | 25397 | −700 | |
| Terser mangle, fixed alphabet | micromark | 84081 | 29794 | 25410 | −687 | |
| relabeled, Terser order | micromark | 87117 | 30386 | 26061 | −36 | `--check` |

