# Library proof matrix

Parent: [evidence](README.md). Measurement contract:
[verification](../verification/README.md). Live numbers:
[`docs/current-status.md`](../../current-status.md). Migration:
[planned migration](../migration/planned-migration.md).

This page defines what must be proved for a real-library compression claim. It
does not copy size results. Tracked generated reports own numbers.

## Required Record

Every artifact row must identify:

1. library/version and immutable source tree;
2. exact supported API/behavior boundary and known exclusions;
3. direct compiler output versus downstream deployment output;
4. application/library world, artifact format, public roots, and ABI fingerprint;
5. LilScript compiler/config/selected recipe and codec fingerprints;
6. independently authored JS source plus eligible baseline tool/options;
7. semantic/API command, pass/skip/fail counts, and artifact hash;
8. selected raw/gzip/Brotli objective and exact selected-metric result;
9. compile wall time, peak memory, stop reason, and starved families;
10. status: eligible win/tie/loss, diagnostic, incomplete, or invalid.

No inherited `dist/` file survives a failed build. A before/after LilScript row
proves regression or recovery only. A downstream minifier proves a joint
deployment pipeline, not direct compiler compression.

## Maintained Pressure Libraries

| Library | Boundary that must be explicit | Required semantic evidence | Current engineering question |
|---|---|---|---|
| MotionLil | Each named ESM/import surface separately; partial surfaces must not be called full Motion | Node API/constructor checks, bundler/tree-shaking checks, browser behavior for DOM surfaces | Why large boundaries regress while `animateMini` improves; whether owned optional/keyframe bags can replace dynamic internals without narrowing the public API |
| MarkedLil | Published package API, parse-only diagnostic, and closed-key diagnostic are separate rows | Official corpus, options/defaults, ESM/CJS/UMD API, closed-key runtime | Replay the previous legal Brotli recipe; distinguish public extern keys from compiler-owned closed keys |
| MobXLil | Regular ESM and shipping production-min are separate artifacts | Full upstream/differential suite, descriptors, Proxy/Reflect/accessors, constructors | Explain regular improvement versus production-min regression without unsafe getter assumptions |
| jQueryLil | Public reusable ESM/CJS/UMD facade, not a closed app | Export surface, Deferred, DOM selectors/classes/events, upstream-compatible browser cases | Reproduce the public boundary; pursue effect-safe value placement and sound array-ness rather than post-hoc ternary/naming retries |

Other libraries join this table only after their boundary and rerunnable harness
exist. A port may be valuable while incomplete, but it is not evidence for APIs
or semantics outside its declared row.

## Completion

For each maintained supported boundary and selected metric:

```text
semantics and ABI pass
and direct LilScript compiler bytes
    <= minimum eligible independently authored JavaScript baseline bytes
```

Strict-win labels require `<`. Cross-metric gains do not offset a selected-metric
loss. Runtime or compile-resource regressions are reported and may block a
non-size objective even when transfer improves.
