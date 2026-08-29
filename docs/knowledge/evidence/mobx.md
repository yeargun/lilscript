# MobXLil evidence

Parent: [evidence](README.md). Required row:
[library proof matrix](library-proof-matrix.md). Live snapshot:
[`docs/current-status.md`](../../current-status.md).

## Boundaries

MobXLil's regular ESM artifact and shipping `production-min` artifact are
separate rows. Both must preserve Proxy, Reflect, accessor/descriptor behavior,
constructor identity, reactions, and the declared package exports.

The maintained upstream/differential suite currently passes all enabled tests
with documented skips. A test that expects assignment to a non-writable property
must execute in strict JavaScript; the same sloppy-mode harness fails against
official MobX and therefore cannot diagnose a runtime difference.

## Evidence Status

The latest local migration slightly improved regular MobX Brotli but materially
regressed `production-min`. The package cannot be described as improved overall.
Tracked evidence must record both artifacts, full semantic gate, selected
recipe, direct compiler bytes, official/Vite baselines, and resource costs.

## Engineering Direction

- replay and recover the previous legal `production-min` representation;
- attribute production-only decisions before adding architecture;
- use proof-backed class/accessor forms where equivalent;
- preserve Proxy/Reflect and descriptors;
- reject global pure-getter assumptions and production-only semantic shortcuts.
