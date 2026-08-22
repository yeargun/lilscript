# LilScript

**LilScript makes JavaScript libraries smaller.**

Site: [yeargun.github.io/lilscript](https://yeargun.github.io/lilscript/)

LilScript is a typed, compression-first language that compiles into JavaScript, and sometimes into an executable (that path will be more stable later). The compiler mangles and reshapes the program into optimized JS that is typically **5–15% smaller** — after gzip, Brotli, or raw — than the best-performing JS toolchains (Oxc, esbuild, Terser, and similar).

## What has been proven

- **Monaco / VS Code’s editor core** — independently compiled monaco-editor-core modules are about **20% smaller** on average (Brotli). The paired IDE `ide.js` is **887,420 → 413,607** Brotli (−53%). Live: [yeargun.github.io/monacolil](https://yeargun.github.io/monacolil/)
- **marked** — parse API of `marked@18.0.10`, 660/660 HTML match. **10,092 → 9,580** Brotli vs parse-only official Oxc (−5.1%), and about **13% faster** on documents in Chromium. Live: [yeargun.github.io/markedlil](https://yeargun.github.io/markedlil/)
- **Zod** — `zod@4.4.3` classic API, 1353/1353 official tests. **54,791 → 34,152** Brotli vs Vite 8 Oxc closer-world (−37.7%). Live: [yeargun.github.io/zodlil](https://yeargun.github.io/zodlil/)
- **Solid 2.0** — official js-framework-benchmark keyed table: **11,180 → 3,862** Brotli (−65%). Live: [yeargun.github.io/solidlil](https://yeargun.github.io/solidlil/)
- **Motion, MobX, jQuery, and smaller complete packages** — same idea, scoped contracts. Motion’s selected surface is **4,044 → 2,333** Brotli. MobX is a small win vs Vite 8 Oxc. jQuery is ported and published; official min is still smaller. Labs: [motionlil](https://yeargun.github.io/motionlil/), [mobxlil](https://yeargun.github.io/mobxlil/), [jquerylil](https://yeargun.github.io/jquerylil/)
- **posthog-js kernel** — UUID, flags, cookies, routing, rate limit, and queue batching from `posthog-js@1.418.10`, not the published IIFE. **3,662 → 3,915** Brotli vs Vite 8 Oxc of that same kernel (**+6.9%**). Smaller raw. Live: [yeargun.github.io/posthoglil](https://yeargun.github.io/posthoglil/)

It works with pretty much any JS/TS library you rewrite. Comparisons vs npm + Vite 8 / Oxc / Terser / Closure are on the [compare page](https://yeargun.github.io/lilscript/compare.html).

## How it compresses JS finer than Vite / Oxc / Terser / esbuild

### 1. By changing the app

```lilscript
class Vector {
  float x;
  float y;

  init(float x, float y) {
    this.x = x;
    this.y = y;
  }

  float lengthSquared() {
    return this.x * this.x + this.y * this.y;
  }
}

int[] values = [1, 2, 3, 4];
auto doubled = values.map((int value) => value * 2);
int sum = 0;
for (int i = 0; i < doubled.length; i++) {
  sum += doubled[i];
}
Vector vector = new Vector(3.0, 4.0);
if (vector.lengthSquared() == 25.0) {
  print(`sum=${sum}`);
}
```

That compiles to:

```js
var b=[1,2,3,4].map(a=>a*2|0);
var a=0,c=0;
while(a<b.length){
  c=c+b[a]|0;
  a=a+1|0;
}
console.log(`sum=${c}`)
```

This is not minification of the same program. The compiler **changed the app**. The class is gone. The method is gone. The vector never escaped, so it scalar-replaced into nothing.

Oxc / esbuild / Terser start from JavaScript and mostly keep the shape of the app. LilScript is designed so the compiler has extra knowledge before JS is spelled — the same idea as Google Closure Compiler Advanced mode, and beyond.

### 2. Tryhard property mangling

Open a large web app (ChatGPT is a good example) and read the shipped JS. You will see a lot of framework properties that never got minified. They stay human-readable.

Those names stay because the toolchain cannot prove they are local. A property might be a public API, a DOM field, a framework hook, or something a plugin reads by string. So the minifier leaves it.

LilScript does two things:

- **(a) Eliminate the objects.** Convenience classes and bags become scalars, arrays, and tight loops — the `Vector` example, at library scale.
- **(b) Rename for the codec.** Names are not just “make everything one letter.” gzip and Brotli win when the same short tokens repeat. The compiler picks high-frequency short names and **scores the finished file** against the compression algorithm you asked for (`raw`, `gzip`, or `brotli`). Different `cost_model` → different names → a different file that is smaller after *that* codec.

That is why the same program can be 5–15% smaller after gzip / Brotli / raw: the JS is shaped and named for the compressor, not for a human reading the AST.

## Try it

Rust 1.85+ recommended. Native output needs a C11 compiler.

```sh
cargo build --release --bins
target/release/lilscript examples/v01.lil
target/release/lilscript examples/v01.lil --target all -o build/v01
```

You pick the objective codec in `lilscript.toml` (`javascript.cost_model`: `raw`, `gzip`, or `brotli`), plus the usual size / performance / compile-time tradeoffs. Full schema: [docs/configuration.md](docs/configuration.md). Language contract: [docs/language-v0.1.md](docs/language-v0.1.md). Why the knobs exist: [docs/knowledge/README.md](docs/knowledge/README.md).

Lilpack is the Vite-based delivery path (`lilpack dev` / `lilpack build`). VS Code + `lilscript-lsp` live in this repo.

## Models, PRs, discussion

Current models can often oneshot a library rewrite in LilScript. On glue-heavy JS, that can land past 20% smaller, sometimes with a bit of runtime improvement (size is the point; perf usually matters less).

PRs and experiments are welcome. LilScript is [MIT](LICENSE.md).

Please do not use LilScript with React.js or any React-related technology. That includes React, React DOM, and official React renderers; frameworks, meta-frameworks, and runtimes built on React such as Next.js, Remix, Gatsby, and Expo; and libraries, bindings, tools, or products whose purpose is to host, wrap, embed, or interoperate with React. Do not use LilScript, or code generated from it, inside a React application or a React-related library. If you are concerned about bundle size, stop using React.

If you have opinions on where the language and compiler should go, open an issue or comment. I would like to discuss it.
