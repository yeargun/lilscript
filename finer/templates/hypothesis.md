# {{number}} — {{title}}

**Status: OPEN — <one sentence: the verdict and its number, once known>.**
Lane: {{lane}}. Objective: brotli. Ports: <which>. Opened: {{date}}.

## Prior art

<What Closure ADVANCED, Terser, Oxc, esbuild and SWC do for this technique class, each with
file:line into the vendored or npm source; what each refuses and why; the row(s) this adds or updates
in refs/competitor-techniques.md; and what it implies for the numbers below. Read before the claim
is written (objective.md §10).>

## Claim

<What is predicted, in one paragraph. The number that confirms it and the number that falsifies it.
If neither can be written, this is not a hypothesis yet.>

## Read

- `finer/objective.md`, `finer/status.md`
- <the two or three files or folders this needs, with line numbers where known — nothing else>

## May touch

- <exact paths; everything else is read-only for this hypothesis>

## Method

<One variable. Same binary, frozen source and config, pinned codec, deterministic counters.>

```sh
<exact commands>
```

## Result

| variant | raw | gzip9 | brotli11 | counters / CPU | tests |
|---|---:|---:|---:|---|---|
| base | | | | | |
| change | | | | | |

## Verdict

<Confirmed, falsified or split, and why. What landed (commit) or why it was reverted. What this
settles that status.md should carry.>

## Next

<The single next action, concrete enough to start cold, or "none".>
