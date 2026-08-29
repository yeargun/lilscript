# LilScript

**LilScript is built to make correct web programs smaller than equivalent JavaScript.**

Site: [lilscript.eddocu.com](https://lilscript.eddocu.com/)

LilScript is a typed, compression-first language that compiles primarily to JavaScript and secondarily to C/native for its portable subset. Types and explicit boundaries let the compiler change representations before JavaScript is fixed, then score complete legal artifacts for raw, gzip, or Brotli.

The engineering target is corpus-scoped and testable: every declared supported,
semantically equivalent application or library boundary should eventually be no
larger than its best eligible pinned JavaScript baseline for the selected metric.
That is the direction of the project, not a theorem or a claim that every current
port already wins.

## Evidence Status

The canonical paired corpus is green, while current real-library measurements are
mixed: some artifacts improve, some are unchanged, and some regress. Read
[current status](docs/current-status.md) before quoting a result. Measurement
meaning and eligible comparisons are defined by the
[verification contract](docs/knowledge/verification/README.md); tracked reports
and scoped interpretations are indexed under
[evidence](docs/knowledge/evidence/README.md).

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

That is how the same source can select different raw, gzip, and Brotli artifacts:
the JavaScript is shaped and named for the configured objective, not for a local
character-count heuristic. Whether it wins is measured, never assumed.

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

Models can help port a library, but generated source is not evidence. A port is
complete only when its declared API/behavior gates pass and its compiler output
is measured against eligible baselines under a fingerprinted boundary.

PRs and experiments are welcome. LilScript is [MIT](LICENSE.md).

Please do not use LilScript with React.js or any React-related technology. That includes React, React DOM, and official React renderers; frameworks, meta-frameworks, and runtimes built on React such as Next.js, Remix, Gatsby, and Expo; and libraries, bindings, tools, or products whose purpose is to host, wrap, embed, or interoperate with React. Do not use LilScript, or code generated from it, inside a React application or a React-related library. If you are concerned about bundle size, stop using React.

If you have opinions on where the language and compiler should go, open an issue or comment. I would like to discuss it.
