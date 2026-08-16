# SolidLil Playwright CPU and RAM validation

Playwright 1.62.1, Chromium 151.0.7922.34; 4000 app and 4000 LSX measured browser interactions. Lower is better. Every observation executes in actual Chromium through Playwright, is paired by randomized block, and is retained. Ratios are SolidLil / official Solid geometric means with 95% paired bootstrap confidence intervals.

## Browser CPU and wall time

| Boundary | Metric | Solid median ms | SolidLil median ms | Ratio [95% CI] | Absolute upper 95% delta | Gate |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Vite app | cold parse/eval/mount wall time | 1.200 | 1.100 | 0.951 [0.920, 0.996] | 0.016 ms | pass |
| Vite app | cold parse/eval/mount main-thread CPU | 1.482 | 1.401 | 0.985 [0.863, 1.129] | 0.437 ms | diagnostic |
| Vite app | first interaction wall time | 0.400 | 0.400 | 0.889 [0.833, 0.949] | -0.019 ms | pass |
| Vite app | first interaction main-thread CPU | 0.625 | 0.574 | 0.914 [0.864, 0.967] | -0.017 ms | pass |
| Vite app | warm steady-state wall time | 13.650 | 9.000 | 0.659 [0.651, 0.668] | -4.525 ms | pass |
| Vite app | warm steady-state main-thread CPU | 13.793 | 9.444 | 0.683 [0.677, 0.690] | -4.273 ms | pass |
| Closure app | cold parse/eval/mount wall time | 1.200 | 1.100 | 0.925 [0.903, 0.947] | -0.059 ms | pass |
| Closure app | cold parse/eval/mount main-thread CPU | 1.454 | 1.366 | 0.911 [0.785, 1.037] | 0.179 ms | diagnostic |
| Closure app | first interaction wall time | 0.400 | 0.300 | 0.787 [0.721, 0.865] | -0.047 ms | pass |
| Closure app | first interaction main-thread CPU | 0.618 | 0.525 | 0.825 [0.769, 0.881] | -0.072 ms | pass |
| Closure app | warm steady-state wall time | 13.500 | 8.800 | 0.650 [0.643, 0.657] | -4.644 ms | pass |
| Closure app | warm steady-state main-thread CPU | 13.696 | 9.258 | 0.674 [0.670, 0.679] | -4.408 ms | pass |
| LSX fixture | cold parse/eval/mount wall time | 5.200 | 4.000 | 0.775 [0.758, 0.804] | -0.916 ms | pass |
| LSX fixture | cold parse/eval/mount main-thread CPU | 5.534 | 4.286 | 0.583 [0.491, 0.691] | -2.452 ms | diagnostic |
| LSX fixture | first interaction wall time | 0.500 | 0.600 | 1.069 [1.006, 1.136] | 0.072 ms | pass |
| LSX fixture | first interaction main-thread CPU | 0.749 | 0.817 | 1.102 [1.044, 1.166] | 0.133 ms | pass |
| LSX fixture | warm steady-state wall time | 61.850 | 60.200 | 0.973 [0.964, 0.984] | -1.019 ms | pass |
| LSX fixture | warm steady-state main-thread CPU | 62.187 | 60.808 | 0.979 [0.970, 0.988] | -0.754 ms | pass |

Warm CPU/wall time gate the upper ratio bound at 1.03×. Cold wall latency uses a 0.25 ms absolute upper bound. Sub-2 ms cold CDP CPU stays diagnostic because unrelated renderer tasks can dominate it even when direct wall latency is stable. First-interaction latency uses a 0.25 ms absolute upper bound so timer quantization cannot turn a tiny absolute difference into a misleading large ratio.

## Chromium JavaScript heap

Four forced Chromium collections precede baseline, cold, live, and disposed snapshots.
JavaScript heap passes when either its 95% ratio upper bound is at most 1.03× or its paired absolute upper difference is at most 131,072 B. This avoids treating a few kilobytes over a small retained baseline as a material regression; the combined managed heap and total-process RSS remain independent gates, while the JS/Oilpan split stays visible.

| Boundary | Phase | Solid median B | SolidLil median B | Comparison [95% CI] | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| Vite app | cold | 109,028 | 114,368 | 1.049 [1.049, 1.049]; Δ upper 5,343 B | pass |
| Vite app | live | 938,478 | 910,760 | 0.971 [0.971, 0.972]; Δ upper -26,188 B | pass |
| Vite app | disposed | 894,722 | 904,784 | 1.012 [1.011, 1.013]; Δ upper 11,589 B | pass |
| Closure app | cold | 106,492 | 109,716 | 1.030 [1.030, 1.030]; Δ upper 3,226 B | pass |
| Closure app | live | 935,328 | 887,360 | 0.949 [0.949, 0.949]; Δ upper -47,873 B | pass |
| Closure app | disposed | 890,486 | 882,400 | 0.991 [0.991, 0.991]; Δ upper -8,076 B | pass |
| LSX fixture | cold | 415,616 | 432,712 | 1.041 [1.041, 1.041]; Δ upper 17,101 B | pass |
| LSX fixture | live | 756,884 | 789,444 | 1.043 [1.042, 1.043]; Δ upper 32,495 B | pass |
| LSX fixture | disposed | 651,228 | 737,288 | 1.132 [1.131, 1.133]; Δ upper 86,255 B | pass |

## Chromium managed heap

This combines JavaScript and Oilpan/embedder heap, allowing an internal bookkeeping trade between those heaps while keeping both components in the report. It passes at 1.03× or a paired absolute upper difference of 262,144 B.

| Boundary | Phase | Solid median B | SolidLil median B | Ratio [95% CI] | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| Vite app | cold | 481,076 | 487,600 | 1.113 [0.979, 1.265]; Δ upper 83,628 B | pass |
| Vite app | live | 1,022,012 | 1,015,392 | 1.010 [0.994, 1.033]; Δ upper 28,838 B | pass |
| Vite app | disposed | 1,094,194 | 1,126,840 | 1.035 [1.014, 1.060]; Δ upper 60,759 B | pass |
| Closure app | cold | 478,540 | 482,948 | 1.028 [0.878, 1.189]; Δ upper 58,171 B | pass |
| Closure app | live | 1,018,928 | 991,992 | 0.987 [0.966, 1.016]; Δ upper 10,171 B | pass |
| Closure app | disposed | 1,089,958 | 1,104,456 | 1.023 [0.999, 1.052]; Δ upper 50,572 B | pass |
| LSX fixture | cold | 883,952 | 872,920 | 0.986 [0.927, 1.046]; Δ upper 36,134 B | pass |
| LSX fixture | live | 1,001,518 | 1,026,762 | 1.015 [0.964, 1.068]; Δ upper 60,123 B | pass |
| LSX fixture | disposed | 1,006,542 | 1,083,428 | 1.073 [1.010, 1.137]; Δ upper 125,837 B | pass |

## Chromium process RSS

RSS sums every Chromium process reported by CDP for the isolated run. Because allocator/page granularity creates zeros and jumps, this is a paired absolute-difference gate with a 4,194,304 B upper allowance.

| Boundary | Phase | Solid median retained B | SolidLil median retained B | Difference [upper 95%] | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| Vite app | cold | 8,552,448 | 8,626,176 | 21,504 B [upper 95,232 B] | pass |
| Vite app | live | 24,895,488 | 22,495,232 | -2,520,064 B [upper -2,260,480 B] | pass |
| Vite app | disposed | 24,944,640 | 22,609,920 | -2,473,472 B [upper -2,205,184 B] | pass |
| Closure app | cold | 8,536,064 | 8,601,600 | -24,064 B [upper 102,912 B] | pass |
| Closure app | live | 25,346,048 | 21,856,256 | -3,930,624 B [upper -3,341,286 B] | pass |
| Closure app | disposed | 25,411,584 | 21,970,944 | -3,885,568 B [upper -3,303,424 B] | pass |
| LSX fixture | cold | 21,626,880 | 17,293,312 | -4,263,936 B [upper -4,002,278 B] | pass |
| LSX fixture | live | 40,828,928 | 32,612,352 | -8,017,920 B [upper -7,702,528 B] | pass |
| LSX fixture | disposed | 41,091,072 | 33,054,720 | -7,829,504 B [upper -7,512,576 B] | pass |

## Ownership and unmount

Every Playwright sample performs idempotent unmount, checks empty application and portal roots, and proves stale controls stop. The deterministic ownership workload separately returns all 8 owner and 16 effect slots with zero pending effects.

## Eligibility

- Vite application: **pass**
- Closure ADVANCED application: **pass**
- Integrated LSX fixture: **pass**
- Browser teardown plus lifecycle slots: **pass**
