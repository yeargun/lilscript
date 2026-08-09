# Exact-entrypoint popular library sizes

Only complete selected entrypoints appear in this comparison. Incomplete research
ports remain implementation backlog and their sizes are deliberately excluded.

Nano ID covers every export of `index.browser.js`, including defaults, coercions,
deterministic custom generators, randomness/distribution, and the 2147483648 callback
step. mitt covers its complete default-export surface and observable runtime function
shape. clsx preserves its default/named identity and recursive raw-JavaScript-value
algorithm without a conversion facade. gl-matrix covers the complete ESM root namespace, every module export and alias,
live `ARRAY_TYPE`, and `setMatrixArrayType` allocation behavior.

Each row uses the same app contract and Vite 8 settings. Adapter bytes are included.
Publication additionally requires differential behavior, no raw or selected-codec
size regression against either npm/Vite 8 or public-API-preserving Closure ADVANCED,
and no material throughput or retained-memory regression. Gzip and Brotli remain
visible because a build tuned for one may trade a few bytes in the other.
Closure ADVANCED receives generated externs for observable published properties, so
it may optimize through the app but may not rename the API being compared.


Solid / solidlil is a partial external row from `lilscript-solid-lab`: Solid JSX
todolist vs solidlil LSX (`.lilx` → LilScript reactive + LilScript DOM), same todo
contract, Vite/oxc-minified full app JS. Brotli 3722 /
5479 (solidlil / Solid; -32.1%). Raw/Terser/Closure columns are not measured in that lab lane.

The Solid/solidlil result above is an application benchmark, not a claim that the
complete Solid package surface has been reimplemented.

| Project | Raw JS | Terser | Closure (actual level) | npm Vite 8 | LilScript raw | LilScript Vite 8 | Brotli (Lil / npm) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Nano ID | 1564 / gz 684 / br 603 | 719 / gz 445 / br 408 | ADVANCED: 750 / gz 461 / br 414 | 732 / gz 455 / br 409 | 1211 / gz 617 / br 549 | 729 / gz 456 / br 408 | 408 / 409 |
| mitt | 1125 / gz 504 / br 452 | 511 / gz 314 / br 284 | ADVANCED: 595 / gz 339 / br 311 | 595 / gz 331 / br 300 | 1061 / gz 481 / br 432 | 595 / gz 329 / br 300 | 300 / 300 |
| clsx | 1906 / gz 732 / br 665 | 1158 / gz 544 / br 490 | ADVANCED: 1158 / gz 555 / br 499 | 1156 / gz 541 / br 493 | 2012 / gz 741 / br 662 | 1153 / gz 538 / br 481 | 481 / 493 |
| gl-matrix | 142374 / gz 22769 / br 17791 | 73693 / gz 17853 / br 14277 | ADVANCED: 73296 / gz 17863 / br 14328 | 73505 / gz 17722 / br 14330 | 118078 / gz 21307 / br 17259 | 68534 / gz 17168 / br 14056 | 14056 / 14330 |
