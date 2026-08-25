# Brotli success after `a5fabd2`

Success is **Brotli-11 of the `cost_model = "brotli"` compile**. gzip-9 and raw are diagnostics. Every library rebuild used that config.

Compiler: `a5fabd2` on `origin/main` (syntax-spelling worktree merged, plus the 24 commits that were already on local `main`). Codec: `lilscript-codec` Brotli quality 11.

The spelling commit itself is **Brotli-neutral** on these ports: `9a0e65b` and `a5fabd2` emitted byte-identical marked / jquery / solid / zod artifacts. Indexed `charAt` search ran and lost. Deltas below are **published github.io (old `origin/main` artifacts) → rebuild on `a5fabd2`**.

## Verdict

| Port | Brotli artifact | Before | After | Δ | Success? |
|---|---|---:|---:|---:|---|
| markedlil | npm ESM (`itslil`, brotli compile) | 9,589 | 9,515 | −74 | yes |
| jquerylil | npm ESM | 30,973 | 30,741 | −232 | yes |
| zodlil | `zod.core.js` closer-world | 34,152 | 31,772 | −2,380 | yes |
| motionlil | `dist/index.js` | 51,339 | 50,938 | −401 | yes |
| motionlil | `dist/mini.js` | 10,113 | 10,003 | −110 | yes |
| mobxlil | production ESM | 16,736 | 17,223 | +487 | no |
| posthoglil | kernel (`itslil` raw, brotli compile) | 5,606 | 5,985 | +379 | no |
| posthoglil | autocapture pack | 3,065 | 3,186 | +121 | no vs last ship; still vs Oxc |
| posthoglil | replay-core pack | 3,432 | 3,465 | +33 | no vs last ship; still vs Oxc |
| solidlil | JFB keyed landing row | 3,862 | 3,862 | 0 | not rebuilt |
| monacolil | IDE / catalog | — | — | — | not rebuilt |

`yes` means smaller Brotli than the previous shipped LilScript file. Oxc comparison is a separate column on the labs.

## Still smaller than Oxc on Brotli?

| Port | Oxc Brotli | LilScript after | vs Oxc |
|---|---:|---:|---|
| marked parse-only | 10,092 | 9,515 | −5.7% |
| jquery official min | 27,445 | 30,741 | +12.0% (still a loss) |
| zod closer-world | 54,791 | 31,772 | −42.0% (core file only; package claim stays withdrawn) |
| mobx Vite 8 Oxc | 17,159 | 17,223 | +0.4% (was −2.5%) |
| posthog kernel Oxc | 5,622 | 5,985 | +6.5% (was −0.3%) |
| posthog autocapture Oxc | 4,215 | 3,186 | −24.4% (was −27.3%) |
| posthog replay-core Oxc | 4,258 | 3,465 | −18.6% (was −19.4%) |

## Not in this rebuild

- **monacolil** — dirty local tree, long compile; github.io left as shipped.
- **solidlil JFB / 16 motion demos** — package files were rebuilt; the landing totals are app bundles, not `dist/*.js`.
- **in-repo ports** (gl-matrix, nanoid, …) — already identical at `9a0e65b` vs `a5fabd2`.

## What this means

The ES-floor / `indexed-char-at` work did not move Brotli on these libraries. Pushing local `main` and reshipping did: marked, jquery, zod core, and motion package files got smaller; MobX and the posthog kernel got larger. gzip/raw of the same brotli-scored files can disagree with that verdict and are ignored here.
