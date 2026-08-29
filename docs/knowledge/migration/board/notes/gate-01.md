# gate-01 — canonical codec contract

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Which direct Node-compressor uses produce size evidence and must move to
`lilscript-codec`, and which are serving/generated/research paths that require a
narrow documented exclusion?

## Why it matters here

`scripts/release-check.sh` runs the codec contract, so every publication runner
must use the pinned implementation rather than a platform-dependent Node codec.

## The five files

- `benchmarks/js-framework-benchmark/adapter/scripts/measure-compression-variants.mjs`
- `benchmarks/js-framework-benchmark/scripts/measure.mjs`
- `benchmarks/popular/apps/monaco/js/ts.worker.js`
- `benchmarks/popular/apps/monaco/lil/ts.worker.js`
- `benchmarks/popular/monaco-layers/serve-ide.mjs`

The two `ts.worker.js` files are vendored build output, and `serve-ide.mjs` compresses
HTTP responses in a dev server rather than producing a size claim. The two `measure*`
scripts are the ones that look like real evidence paths. That split is a hypothesis
from reading imports, not a verdict — confirm per file before changing either side.

## Constraints specific to this task

- Do not relax the pattern list to make the test pass. If a file is genuinely not size
  evidence, the contract should say so by path, narrowly, with the reason in the test.
- Vendored build output should be excluded as vendored, not by pattern.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Contract test state | `node --test benchmarks/codec-contract.test.mjs` | 9 pass, 1 fail — "benchmark and publication runners use only the canonical codec wrapper", 5 paths | diag |
| 2026-08-19 | Pre-existing at HEAD, not from working-tree edits | `git show HEAD:<path> \| grep -cE "zlib\|gzipSync\|brotliCompressSync"` | 7 / 5 / 7 matches in the three `.mjs` files at HEAD | diag |
| 2026-08-29 | Canonical wrapper and reviewed exclusions | `node --test benchmarks/codec-contract.test.mjs` | 10/10 pass | gate |

## Log

- 2026-08-19 — Found while checking that `scripts/board.mjs` itself passes the scanner;
  it does. The failure predates this session and predates the working-tree edits. — **OPEN**
- 2026-08-29 — Measurement scripts use the canonical wrapper; vendored,
  research, and serving-only paths have narrow reviewed exclusions. — **LANDED**

## Next step

Keep `node --test benchmarks/codec-contract.test.mjs` green when adding or moving
any benchmark/publication runner.
