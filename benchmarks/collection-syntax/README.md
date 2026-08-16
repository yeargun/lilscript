# Collection syntax benchmark

This gate covers array and record spread, nullable destructuring, array and
record rest, and callback-free `for...of`. It compares the compact syntax with
an explicit loop/copy spelling under separate gzip and Brotli candidate-search
configs, executes both JavaScript and native artifacts, and checks the selected
implementation against a direct JavaScript runtime/memory reference.

Run `node benchmarks/collection-syntax/run.mjs`.
