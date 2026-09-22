# @itslil/mdast-util-to-hast

Official [`mdast-util-to-hast@13.2.1`](https://github.com/syntax-tree/mdast-util-to-hast) algorithms rewritten in LilScript. Official test suite 148/148. Not affiliated with upstream.

**Site:** [yeargun.github.io/mdast-util-to-hastlil/](https://yeargun.github.io/mdast-util-to-hastlil/)

```sh
npm install @itslil/mdast-util-to-hast
```

Two compiles ship from the same `.lil` source:

The runtime is bundled, so `src/convert.lil` groups the upstream handlers, state, and footer for whole-program optimization. The declaration file likewise merges upstream's root and public `lib` types into one artifact while retaining the exact public API.

| Lane | Config | Meaning |
| --- | --- | --- |
| **library** (npm) | `lilscript.toml` · `--target js-module` | reusable ESM. Export names and `extern class` keys stay. |
| **closed** | `lilscript.closed.toml` · `--target js-module` | closed LilScript world. `extern class` keys may mangle. ESM export names stay so the lane is testable. |

You publish the library lane. `dist/to-hast.closed.js` is diagnostic only.

The LilScript compiler lives next door at `../lilscript`.
