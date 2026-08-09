# Exact-library configuration effort audit

Every artifact below passes its reusable-module API/behavior check. Times include
compiler process startup and are intended to show the tuning curve, not a stable
cross-machine compiler benchmark. Sizes are the emitted ESM module before Vite.

| Library | Profile | Priority / level | Candidate cap / beam | Compile ms | Raw | gzip-9 | Brotli-11 |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| nanoid | fast | performance-first / 0 | 1 / 1 | 30.6 | 594 | 407 | 347 |
| nanoid | balanced | balanced / 8 | 64 / 4 | 1470.4 | 548 | 387 | 330 |
| nanoid | size | size-first / 12 | 256 / 6 | 5457.1 | 514 | 368 | 317 |
| nanoid | maximum | size-first / 15 | 1536 / 12 | 13926.2 | 514 | 370 | 319 |
| mitt | fast | performance-first / 0 | 1 / 1 | 8.0 | 465 | 235 | 201 |
| mitt | balanced | balanced / 8 | 64 / 4 | 531.4 | 319 | 193 | 163 |
| mitt | size | size-first / 12 | 256 / 6 | 2033.6 | 319 | 193 | 163 |
| mitt | maximum | size-first / 15 | 1536 / 12 | 10996.6 | 319 | 193 | 163 |
