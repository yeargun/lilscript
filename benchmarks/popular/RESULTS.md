# Exact-entrypoint popular library sizes

Only complete selected entrypoints appear in this comparison. Incomplete research
ports remain implementation backlog and their sizes are deliberately excluded.

Nano ID covers every export of `index.browser.js`, including defaults, coercions,
deterministic custom generators, randomness/distribution, and the 2147483648 callback
step. mitt covers its complete default-export surface and observable runtime function
shape. clsx preserves its default/named identity and recursive raw-JavaScript-value
algorithm without a conversion facade. gl-matrix covers the complete ESM root namespace, every module export and alias,
live `ARRAY_TYPE`, and `setMatrixArrayType` allocation behavior. motion measures the selected Motion 13
`mix`/`wrap`/`stagger`/`spring` surface used by the app (same equations as npm `motion@13`); full DOM
package completeness remains the compatibility backlog (React entrypoints are out of scope).

Each row uses the same app contract and Vite 8 settings. Adapter bytes are included.
Publication additionally requires differential behavior and no selected-codec
size regression against either npm/Vite 8 or public-API-preserving Closure ADVANCED,
and no material throughput or retained-memory regression. Gzip and Brotli remain
visible, along with raw, as diagnostics because a build tuned for one may trade
bytes in the others. The current publication objective is Brotli, so only the
Brotli cell of the Brotli-selected LilScript artifact is size-gated.
Closure ADVANCED receives generated externs for observable published properties, so
it may optimize through the app but may not rename the API being compared.


Solid / solidlil is an archived sibling-worktree snapshot: Solid JSX todolist
vs solidlil LSX (`.lilx` → LilScript reactive + LilScript DOM), same todo
contract, Vite/oxc-minified full app JS. Brotli 3722 /
5479 (solidlil / Solid; -32.1%). The current integrated lab does not contain the LSX pipeline, so this row
is historical evidence rather than a reproducible single-repository gate.

The Solid/solidlil result above is an application benchmark, not a claim that the
complete Solid package surface has been reimplemented.

| Project | Raw JS | Terser | Closure (actual level) | npm Vite 8 | LilScript pre-Vite (diagnostic triplet) | LilScript Vite (Brotli objective) | Brotli (Lil / npm) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Nano ID | 1564 / gz 681 / br 603 | 719 / gz 447 / br 408 | ADVANCED: 750 / gz 461 / br 414 | 732 / gz 456 / br 409 | 1211 / gz 615 / br 552 | 735 / gz 459 / br 408 | 408 / 409 |
| mitt | 1125 / gz 503 / br 452 | 511 / gz 311 / br 284 | ADVANCED: 595 / gz 336 / br 311 | 595 / gz 331 / br 300 | 1055 / gz 499 / br 447 | 597 / gz 334 / br 302 | 302 / 300 |
| clsx | 1906 / gz 735 / br 665 | 1158 / gz 539 / br 490 | ADVANCED: 1158 / gz 546 / br 499 | 1156 / gz 536 / br 493 | 1976 / gz 724 / br 646 | 1151 / gz 542 / br 497 | 497 / 493 |
| gl-matrix | 142374 / gz 22693 / br 17791 | 73693 / gz 17744 / br 14277 | ADVANCED: 73296 / gz 17747 / br 14328 | 73505 / gz 17646 / br 14330 | 116878 / gz 21357 / br 17352 | 68496 / gz 17110 / br 14116 | 14116 / 14330 |
| motion (mix/wrap/stagger/spring) | 30150 / gz 7852 / br 7116 | 10169 / gz 4500 / br 4081 | ADVANCED: 8908 / gz 4183 / br 3810 | 10356 / gz 4348 / br 4044 | 10332 / gz 3417 / br 2982 | 5495 / gz 2606 / br 2333 | 2333 / 4044 |
