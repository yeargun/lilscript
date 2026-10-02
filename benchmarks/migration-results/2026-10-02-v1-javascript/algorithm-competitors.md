# Escalating algorithm compression corpus

11 selected algorithms; 1 passed and 10 failed with 22 failure events.

| Algorithm | Tier | Functions/modules/depth | Lil raw relation JS | Lil gzip relation JS | Lil Brotli relation JS | Gate |
|---|---|---:|---:|---:|---:|---|
| aggregate-ledger | small-structural | 6/1/3 | 257 ≤ 221 (closure-advanced) | 187 ≤ 182 (closure-advanced) | 171 ≤ 160 (closure-advanced) | FAIL |
| collection-geometry | small-structural | 6/1/2 | 317 ≤ 283 (closure-advanced) | 225 ≤ 210 (closure-advanced) | 197 ≤ 198 (closure-advanced) | FAIL |
| dictionary-template-router | medium-structural | 10/2/4 | 615 ≤ 594 (closure-advanced) | 355 ≤ 373 (closure-advanced-module-graph) | 316 ≤ 335 (closure-advanced) | FAIL |
| helper-sharing | medium-structural | 8/2/5 | 216 ≤ 208 (closure-advanced-module-graph) | 175 ≤ 170 (closure-advanced-module-graph) | 158 ≤ 149 (closure-advanced) | FAIL |
| large-event-analytics | large-structural | 22/6/7 | 810 ≤ 853 (closure-advanced) | 473 ≤ 489 (closure-advanced) | 427 ≤ 425 (closure-advanced) | FAIL |
| policy-specialization | medium-structural | 8/2/4 | 312 ≤ 290 (closure-advanced-module-graph) | 216 ≤ 204 (closure-advanced-module-graph) | 195 ≤ 179 (closure-advanced-module-graph) | FAIL |
| shape-invoice-pipeline | medium-structural | 10/2/5 | 477 ≤ 452 (closure-advanced-module-graph) | 287 ≤ 298 (closure-advanced-module-graph) | 263 ≤ 261 (closure-advanced-module-graph) | FAIL |
| state-machine-parser | small-structural | 7/1/3 | 353 ≤ 352 (closure-advanced) | 269 ≤ 263 (closure-advanced) | 245 ≤ 244 (closure-advanced) | FAIL |
| stateful-packet-decoder | medium-structural | 11/2/5 | 547 ≤ 540 (closure-advanced-module-graph) | 344 ≤ 341 (closure-advanced-module-graph) | 317 ≤ 313 (closure-advanced-module-graph) | FAIL |
| static-rule-engine | small-structural | 5/1/4 | 307 ≤ 326 (closure-advanced) | 205 ≤ 215 (closure-advanced) | 184 ≤ 188 (closure-advanced) | pass |
| string-dictionary | small-structural | 5/1/3 | 432 ≤ 403 (closure-advanced) | 271 ≤ 286 (closure-advanced) | 239 ≤ 246 (terser) | FAIL |
