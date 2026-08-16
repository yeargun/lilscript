# Results

Release gate result (Node 22.21.1, gzip level 9, Brotli quality 11): PASS.

> Historical snapshot: this report predates the explicit raw-cost-model lane.
> Regenerate it before using the raw cell as current objective-specific evidence.

| Artifact | Raw | gzip | Brotli |
| --- | ---: | ---: | ---: |
| Compact generator star on | 484 B | 234 B | 189 B |
| Compact generator star off | 494 B | 236 B | 192 B |
| Manually minified native-JS reference | 610 B | 300 B | 259 B |

Median runtime across 21 alternating isolated runs was 45.260 ms on, 45.429 ms off, and
45.506 ms for the reference. Median retained heap was 148,384 B on, 148,408 B off, and 151,752 B
for the reference. Output and edge fixtures matched exactly.

The current external JavaScript comparison uses independent raw-, gzip-, and
Brotli-selected LilScript artifacts for the corresponding metrics; it is not a
claim that one artifact wins all metrics. The historical spelling comparison
saves 10 raw bytes, 2 gzip bytes, and 3 Brotli bytes in its pass-isolation
artifacts. It measured
0.4% faster than the disabled spelling and 0.5% faster than the structurally
different reference in this run, while retaining 24 fewer bytes than the
disabled spelling and 3,368 fewer than the reference. The on/off change is only
generator-token whitespace, so it does not change the executed function body;
the larger sample reduces process-order noise. The full flattened Lilscript
artifact is also 126 raw, 66 gzip, and 70 Brotli bytes smaller than the
already-minified native-class reference. All runtime and memory comparisons use
a strict 5% gate.
