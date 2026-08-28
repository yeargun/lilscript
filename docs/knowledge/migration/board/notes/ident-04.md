# ident-04 — freeze identity as paired canonical folders

Parent: [ledger](../LEDGER.md). Status: landed. Depends on [ident-03](ident-03.md).

## Question

What is the permanent, reviewed record that this class stays fixed?

## Current hypothesis

A `canonical/identity/` family under `comparison/cases/`, one folder per shape, `expect = "le"`.
These are correctness-first: stdout must match, and compressed size must not lose to the
best valid Terser/Oxc/esbuild artifact. Independently authored JS uses `{__proto__: null, ...}`
so the pair matches LilScript `Record` (null prototype). The five shapes are snapshot-write,
snapshot-rebind, snapshot-computed, snapshot-captured-rebind, and saved-loop-phi.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | Identity family vs minifiers | `LILSCRIPT=target/debug/lilscript LILSCRIPT_CODEC=target/debug/lilscript-codec node comparison/cases/run.mjs --only identity/` | `verified 5/5 cases`; strict wins raw=5, gzip=5, brotli=4 | gate |

## Log

- 2026-08-19 — Opened as the durable half of the identity lane. — **OPEN**
- 2026-08-28 — Five folders under `comparison/cases/canonical/identity/`. Production
  CLI rematerialized a top-level captured rebind (`94` not `89`) until callee flush
  treated globals like locals; record index `??null` now uses `NullNormalized` so
  `values["k"]??0` is `values.k??0`; unused assignment IIFEs beta-reduce. — **LANDED**

## Next step

None for this note. 07.1 identity is closed. Continue as [arch-02](arch-02.md).
