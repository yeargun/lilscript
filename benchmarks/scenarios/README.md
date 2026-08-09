# Real-application and mangling matrix

These scenarios compose complete, separately verified npm-compatible LilScript
ports into application-shaped logic. They do not replace the reusable-library
benchmarks: whole-program specialization is expected here and is part of what
the lab is measuring.

The additional `property-ledger` fixture is not presented as an application or
npm comparison. It deliberately sends records to a separately loaded host whose
contract observes values but not keys, preventing scalar replacement while
making closed-world property renaming semantically legal and measurable.

Every artifact must print the checked `expected.txt` value before it is
measured. The lanes deliberately separate bundling, identifier mangling, and
property mangling:

- **Vite 8 / no minify** is a Rolldown bundle with `minify: false`.
- **Vite 8 / Oxc default** is Vite's normal production minifier. Oxc currently
  mangles bindings, not ordinary public object properties.
- **Vite 8 / Terser private properties** mangles only names beginning with `_`.
  Those fields are closed-world implementation details in all three apps.
- **Closure ADVANCED** receives the same unminified npm bundle. The app exposes
  only its printed contract, so whole-program renaming is valid.
- **LilScript / mangling off** disables optimization and all mangling. This is
  also the native/C behavior-verification lane; native bytes are not compared
  with JavaScript bytes.
- **LilScript / public-safe** enables maximum optimization and identifier
  mangling while preserving aggregate/export names.
- **LilScript / closed world** additionally allows aggregate/export property
  renaming. It is valid only because no object crosses the app boundary.
- **LilScript closed world + Vite/Oxc** shows whether a normal Vite deployment
  finds anything else after LilScript's own compressor.

Raw, gzip-9, and Brotli-11 are reported independently. No lane is called a win
unless it has the same behavior boundary.
