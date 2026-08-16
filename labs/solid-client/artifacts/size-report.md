# Bundle size report

Brotli-11 transfer bytes are the primary release gate. Gzip-9 and raw bytes remain explicit diagnostics.

| Comparison | Brotli-11 · primary | Gzip-9 | Raw |
| --- | --- | --- | --- |
| closed-world Vite app | pass | pass | pass |
| closed-world Closure app | pass | pass | pass |
| open-world core API | pass | pass | tradeoff |
| closed-world LSX parity fixture | pass | pass | pass |

| Artifact | Brotli-11 · primary | Gzip-9 | Raw |
| --- | ---: | ---: | ---: |
| solid-vite | 4466 | 4956 | 12522 |
| solid-closure-advanced | 4329 | 4797 | 11270 |
| lilscript-vite | 3799 | 4234 | 10582 |
| lilscript-closure-advanced | 3836 | 4285 | 10474 |
| lilscript-compiler | 2847 | 3146 | 7780 |
| solid-lsx-vite | 12560 | 13866 | 38629 |
| solidlil-lsx-vite | 10741 | 12035 | 34209 |
| solid-core-open | 8551 | 9422 | 29474 |
| solidlil-core-open | 8433 | 9340 | 30074 |
