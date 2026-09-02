# 041 — measurements

Run details and the full tables for [README.md](README.md). Everything under `finer/out/041/`.

### As run (2026-09-02, 00:10–03:30)

- **B**, no build. `converge_local_names_over_an_artifact_file` (`src/js_peephole/tests.rs`) reads
  `LILSCRIPT_RENAME_INPUT`, runs the pass, writes `LILSCRIPT_RENAME_OUTPUT`, prints which bail fired
  and every unsound scope and ambiguous name (`step-b-test.log`, `step-b-artifacts.log`). Headers by
  `headers.mjs` (acorn 8.15.0, the count 039 used: every function whose parameters are all plain
  identifiers, two or more of them). Sizes from `lilscript-codec --json`. Tests: the port's six compat
  tests copied unmodified into `jqtest/` with the port's `node_modules` symlinked, `node --test`, plus
  `node --check`.
- **A**, after the fleet. Two jquerylil builds from the port's committed source and config (`git
  archive HEAD` into `/tmp`, `node_modules` shared), one binary each from one source (cc18452):
  `base` = `git archive HEAD` of the compiler built into a `/tmp` target dir, `narrowed` =
  `target/release/lilscript` from the tree with the counters and `narrow-the-bail.patch` applied.
  `taskset` 0-3 / 4-7, `RAYON_NUM_THREADS=4`, `LILSCRIPT_TIMING=1`; the JSON is
  `step-a-<label>-timing.json`, the artifacts `jquery.esm.<label>.js`. Build windows 01:20–03:02 and
  01:22–03:04 are wall clock on a shared host and not results.
- The suite on the tree as left: `suite.log`, 1660 passed / 0 failed / 0 ignored, 9 binaries.
- `step-c-fleet.sh` and `fleet-diff.mjs` were written and not run; `scoreboard.baseline.json` is the
  fleet pass of 2026-09-02 01:16 (jquerylil FAILED on its 90-minute timeout there, so its row is the
  18:15 dist).

## B — the pass over finished artifacts

| variant | rewrites | distinct multi-param headers | raw | gzip9 | brotli11 | tests |
|---|---:|---:|---:|---:|---:|---|
| tree artifact (039 copy) | — | 90 | 83778 | 32540 | 29270 | 6/6 |
| shipped pass over it: bail at `rename.rs:46-48` (0 unsound scopes, 6 ambiguous names) | 0 | 90 | 83778 | 32540 | 29270 | identical |
| pass with that bail narrowed to unsound scopes, over it | 5880 | **25** | 83470 | 31759 | **28505 (−765)** | 6/6, `--check` |
| Terser `mangle` locals-only over it (039) | — | 22 | 83375 | 31737 | 28505 | |
| committed artifact | — | 25 | 83044 | 31530 | 28225 | 6/6 |
| shipped pass over the committed artifact: the same bail, 6 ambiguous names | 0 | 25 | 83044 | 31530 | 28225 | identical |
| narrowed pass over the committed artifact | 836 | 25 | 83044 | 31550 | 28233 (+8) | 6/6, `--check` |
| mobx dist copy: 1 unsound scope, a rest parameter `(s,p,...c)` | 0 | | | | | |
| micromark dist copy: 4 template literals | 0 | | | | | |

The six ambiguous names, tree artifact (byte offsets): `t` at 1159 (`var t=e.nodeType;if(!t){var
r,t="",a=0;…`), `a` at 6112 (`var c=r,a=!0,e,h,s,g,m`), `o` at 11065 (`var o=[];…for(var
r=[],e=t[0],o=0,a=1;…`), `t` at 12144, `i` at 56188, `i` at 57257 (`for(var n=t.length,i=!1,r=0;…`).
The committed artifact has the same six shapes at 1030, 5811, 11281, 15054, 56049, 57102.

## A — counters and artifacts

| counter, narrowed build | value |
|---|---:|
| `cleanup_entered` / `cleanup_unbudgeted` (exit at `compiler.rs:7681`, ledger empty) / `cleanup_skipped` | 10 / **19** / 0 |
| `rename_candidates` (reached the loop, got a work unit) | 20 |
| `rename_starved` (break at `:7764`) | **0** |
| `rename_ambiguous` (the shipped bail would have fired; a duplicate `var`, no unsound scope) | **20 of 20** |
| `rename_idle` / `rename_unparsed` / `rename_refused` / `rename_unprobed` | 0 / 0 / 0 / 0 |
| arrivals at the vote (`:7783`): `rename_won` (Σ saved) / `rename_lost` (Σ lost) | **15** (8681) / 5 (196) |
| `codec_calls` (base: 5036) | 5068 |

| jquerylil build | distinct headers | raw | gzip9 | brotli11 | tests |
|---|---:|---:|---:|---:|---|
| base (HEAD compiler) | 91 | 83837 | 32564 | 29304 | 6/6, `--check` |
| narrowed, as shipped | — | 82596 | 31879 | 28567 | **`node --check` fails** at one site |
| narrowed, the two parentheses restored by hand (`jquery.esm.narrowed.repaired.js`) | 40 | 82598 | 31878 | **28571 (−733)** | 6/6, `--check` |
| the 18:15 dist the fleet scores (an older binary) | 90 | 83778 | 32540 | 29270 | 6/6 |

The invalid site, byte 40725 of the narrowed artifact. Tree: `…?e.getClientRects().length>0?(a="border-box"===d(e,"boxSizing",!1,i),l=s in e,l&&(n=e[s])):l=a:l=a,…`.
Narrowed: `(m&&o||g||s||c)&&e.getClientRects().length>0?o="border-box"===i(e,"boxSizing",!1,a),s=u in e,s&&(r=e[u]):s=o,…`.
`node --check` on all 22 fleet dists: only mobxlil fails, on `export` in CommonJS mode — a false positive, not this shape.

## C — fleet A/B, the narrowing landed behind the 044 fix (2026-09-02)

One host, two full passes of the fleet on the same port sources: A = the HEAD (54ab05a) binary,
B = the same tree with 044's fold fix and validator and `narrow-the-bail.patch` applied
(`finer/out/044/`: `scoreboard.base.json`, `scoreboard.new.json`, `fleet-diff.base-vs-new.md`, the
build logs). The A pass reproduced the 01:16 baseline byte for byte on 18 ports (rehype-katexlil −9
is the 4117ea2 build-script fix), so the two binaries are the one variable. jquerylil and markedlil
were built outside the fleet on four cores each (`build-port.sh`), jquerylil's B in the port's
tree. Sizes from `lilscript-codec`; build seconds are wall clock on a contended host, not a result.

| port | A brotli11 | B brotli11 | Δ brotli | Δ raw | port tests on B |
|---|---:|---:|---:|---:|---|
| remarklil | 39333 | 37239 | **−2094** | −1307 | 504/504 (`node --test`; `check:sources`/types not run) |
| rehypelil | 52462 | 51696 | **−766** | +1362 | 159/159 |
| remark-gfmlil | 10836 | 10559 | **−277** | −464 | 19/19 |
| mdast-util-to-hastlil | 4264 | 4262 | −2 | +3 | 149/149 |
| markedlil (`marked.esm.js`, Brotli) | 9506 | 9444 | **−62** | −15 | 29/29; every lane under its own objective: `marked.raw.js` (Brotli config) 9423→9392, gzip lane gzip9 10574→10502, bytes lane raw 33537→33537, closed 9341→9278; the purity diagonal holds on B |
| jquerylil (`jquery.esm.js`, level 15 `always`) | 29304 | 28641 | **−663** | −1235 | `--check`, 6/6, `animate` smoke ok (`scrollTop(1)` TypeError pre-existing, on A too); A rebuilt byte-identical to 041's `jquery.esm.base.js` (sha 938cba…) |
| react-markdownlil, katexlil, micromarklil, remark-parselil, mdast-util-from-markdownlil, mobxlil, unifiedlil, remark-mathlil, posthoglil, remark-breakslil, remark-rehypelil, rehype-stringifylil, hast-util-to-htmllil, zodlil, rehype-katexlil | — | — | 0 | 0 | byte-identical |
| monacolil, playcanvaslil | — | — | — | — | fail to build on both sides (`ERR_MODULE_NOT_FOUND`) |
