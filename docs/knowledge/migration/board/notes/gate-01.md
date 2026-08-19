# gate-01 — the codec-contract gate is red at HEAD

Parent: [ledger](../LEDGER.md). Status: todo.

## Question

Five runners import Node's compressors directly, which the codec contract forbids.
Is each one size *evidence* (must move to `lilscript-codec`), or is it serving/
diagnostic use that the contract should name as allowed?

## Why it matters here

`scripts/release-check.sh:28` runs `node --test benchmarks/codec-contract.test.mjs`,
so the release gate is red before the identity lane touches anything. A fresh context
that runs the suite will see a failure that has nothing to do with the work it was
asked to do, and may "fix" it by weakening the contract — which is
[refusal 3](../README.md#standing-refusals).

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

## Log

- 2026-08-19 — Found while checking that `scripts/board.mjs` itself passes the scanner;
  it does. The failure predates this session and predates the working-tree edits. — **OPEN**

## Next step

Classify each of the five paths as evidence or not-evidence, and record the answer here
before editing anything. Only then either port the evidence paths onto
`benchmarks/codec-contract.mjs` / `lilscript-codec`, or add a narrow, reasoned exclusion
for the non-evidence ones.
