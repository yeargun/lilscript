# recover-01 — MobX production-min incumbent

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Which first generic compiler decision made the frozen legal MobX
`production-min` artifact unreachable or outranked while regular MobX improved?

## Current hypothesis

The reported loss compared different boundaries and had no valid frozen
compiler incumbent. On the first reproducible true production-min boundary,
`06b89aa` improves over exact `2d2268` by 521 Brotli bytes.

## Constraints specific to this task

Retain regular MobX, Proxy, Reflect, descriptors, constructors, and the current
candidate. No package/path matcher, unsafe getter assumption, wider search budget,
or production-only semantic change.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Starting triage state | `docs/current-status.md`; tracked MobX evidence and Git history | provisional production-min loss is +1,230 Brotli while regular MobX is -23; regenerate only after attribution | diag |
| 2026-08-29 | Historical artifact audit | canonical codec over Git blobs `0b29ddc`, `e14a5a0`, `1c59b75`, and current `dist` | Brotli 16,736 / 14,782 / 15,083 / 15,083; `e14a5a0` predates `config/production.min.toml`, and `1c59b75` has no compiler identity | gate |
| 2026-08-29 | Exact first reproducible true lane | build LilScript `2d2268`; compile pinned MobX source with `config/production.min.toml`; canonical codec | raw 58,785, gzip-9 17,853, Brotli-11 16,012; 66.14 s | gate |
| 2026-08-29 | Exact migration lane | gate-02 `06b89aa` observation | Brotli-11 15,491, a 521-byte improvement over exact `2d2268`; semantics passed with 78 exports | gate |
| 2026-08-29 | Historical semantics | `node comparison/large-libraries/semantic/mobx-lane.mjs --root /home/azureuser/mobxlil --artifact /tmp/opencode/mobx-2d-production-min.mjs` | passed with 78 exports | gate |

## Log

- 2026-08-29 — V-01 implementation is complete under targeted checks; G2 promotion is explicitly deferred. Started phase-1 attribution without treating the deferred gate as passed. — **OPEN**
- 2026-08-29 — Refuted `exported-internal-inlining`: the exact production allowlist contains `ir-inlining-variants`, not the separately gated `exported-internal-inlining` decision. Refuted pristine-builtins as recovery: the unsafe diagnostic worsened to 16,530 Brotli. — **REJECTED**
- 2026-08-29 — The +1,230 row compared current true production-min with the older regular production artifact. The first true committed min artifact is compiler-unpinned. Exact `2d2268` → `06b89aa` improves 16,012 → 15,491, so there is no reproducible legal incumbent regression to recover. — **LANDED**

## Next step

Keep regular and production-min as separate matrix rows and reject any future
comparison that does not pin the true config and exact compiler.
