# LilScript

**One compiler, 2026-09-24.** The route that shared the binary with it until 2026-09-23 is deleted (plan phase M1). The architecture is [docs/future-architecture.md](docs/future-architecture.md), the only plan is [docs/migration/index.md](docs/migration/index.md), and what stands today is [docs/current-status.md](docs/current-status.md). Implementation gates remain unverified.

**LilScript is built to make correct web programs smaller than equivalent JavaScript.**

Site: [lilscript.eddocu.com](https://lilscript.eddocu.com/)

LilScript is a typed, compression-first language that compiles primarily to JavaScript and secondarily to C/native for its portable subset. Types and explicit boundaries let the compiler change representations before JavaScript is fixed, then score complete legal artifacts for raw, gzip, or Brotli.

The engineering target is corpus-scoped and testable: every declared supported,
semantically equivalent application or library boundary should eventually be no
larger than its best eligible pinned JavaScript baseline for the selected metric.
That is the direction of the project, not a theorem or a claim that every current
port already wins.

> **Branch note (2026-09-01).** `main` was reset to the line where the active work lives; the
> previous `main` is preserved as `main-pre-2026-09-01`. The two lines had run in parallel from
> `bb413e0` since 08-29. See [BRANCH-HISTORY.md](BRANCH-HISTORY.md) for what each side contains and
> what still needs reconciling.

## Evidence Status

Mixed, and stated per library. On the goal boundaries (six reference ports and
the react-markdown family), the one compiler beats the strongest pinned bar on 11
of 13 in patched scratch builds; motionlil and zodlil still lose. On small closed
programs it is behind the deleted route: the canonical paired micro corpus
(`comparison/cases`) is 9 wins, 6 ties and 38 losses under Brotli against the
smallest competitor, where the old route was 53/1/0. Read
[current status](docs/current-status.md) before quoting a result. Measurement
meaning and eligible comparisons are defined by the
[verification contract](docs/knowledge/verification/README.md); tracked reports
and scoped interpretations are indexed under
[evidence](docs/knowledge/evidence/README.md).
The compiler and the language follow [the architecture](docs/future-architecture.md)
and its [migration plan](docs/migration/index.md). Open language decisions remain explicit. Former migration plans are retired;
[finer/](finer/README.md) retains measurement tools and historical experiments.

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

With `strip_console = false` (as the repository's `lilscript.toml` sets, so `print` stays), the compiler route deleted on 2026-09-23 compiled it to:

```js
for(var a=[1,2,3,4].map(n=>n*2|0),r=0,o=0;r<4;r++)o=o+(a[r]|0)|0;console.log(`sum=${o}`)
```

This is not minification of the same program. That compiler **changed the app**. The class is gone. The method is gone. The vector never escaped, so it scalar-replaced into nothing, and the known length folded into the loop.

The one compiler does part of this today: the class and its method dissolve into a plain object and two free functions, but the vector is not yet scalar-replaced and the length is not folded:

```js
let e=(a,b,c)=>{a.x=b;a.y=c},f=a=>a.x*a.x+a.y*a.y,d=[1,2,3,4].map(a=>a*2|0),b=0;for(let a=0;a<(d.length|0);a=a+1|0)b=b+(d[a]|0)|0;let g={x:0,y:0};e(g,3,4);if(f(g)==25)console.log(`sum=${b}`);
```

The typed interprocedural rules that closed that gap on the old route (inlining, scalar replacement, constant folding) are being rebuilt as facts and rules on the one compiler's program model, for JavaScript and C alike: plan phases M6 and M7.

Oxc / esbuild / Terser start from JavaScript and mostly keep the shape of the app. LilScript is designed so the compiler has extra knowledge before JS is spelled — the same idea as Google Closure Compiler Advanced mode, and beyond.

### 2. Tryhard property renaming

Open a large web app (ChatGPT is a good example) and read the shipped JS. You will see a lot of framework properties that never got minified. They stay human-readable.

Those names stay because the toolchain cannot prove they are local. A property might be a public API, a DOM field, a framework hook, or something a plugin reads by string. So the minifier leaves it.

LilScript does two things:

- **(a) Eliminate the objects.** Convenience classes and bags become scalars, arrays, and tight loops — the `Vector` example, at library scale. The one compiler dissolves classes today; scalar replacement on an escape fact is plan M6.6 and M7.9.
- **(b) Rename for the codec.** Private properties are renamed only where the checker proves who owns them; that typed property renaming is plan M9.6, and today every property keeps its name. Local names are chosen per artifact. Names are not just “make everything one letter.” gzip and Brotli win when the same short tokens repeat. The compiler picks high-frequency short names and **scores the finished file** against the compression algorithm you asked for (`raw`, `gzip`, or `brotli`). Different `cost_model` → different names → a different file that is smaller after *that* codec.

That is how the same source can select different raw, gzip, and Brotli artifacts:
the JavaScript is shaped and named for the configured objective, not for a local
character-count heuristic. Whether it wins is measured, never assumed.

## Try it

Rust 1.85+ recommended. Native output needs a C11 compiler (`CC`, default `cc`).

```sh
cargo build --release --bins
target/release/lilscript examples/v01.lil                      # JavaScript to stdout
target/release/lilscript examples/v01.lil -o build/v01.js
target/release/lilscript src/lib.lil --target js-module -o dist/lib.js
target/release/lilscript examples/v01.lil --target all -o build/v01   # .js, .c and a native executable
target/release/lilscript examples/v01.lil --print-policy       # the resolved policy, then exit
```

| Flag | Meaning |
|---|---|
| `--target js \| js-module \| c \| native \| all` | A closed script, a library whose exports are the API, C11, a native executable, or all of them |
| `-o, --output` | Output file, or the base path for `--target all` |
| `--config` | An explicit `lilscript.toml`; otherwise it is discovered from the input's directory upward |
| `--mode development` | Skip the candidate search (Lilpack's dev server uses it) |
| `--explain human \| json` | The compiler's report on stderr |
| `--print-policy` | The resolved policy (contract, objective, effort, tactic permissions, retired-key warnings) with its fingerprint |
| `--write-lock` | Rewrite `lilscript.lock` from the declared dependency graph |

There is one compiler: `--backend` is refused. In `lilscript.toml` you pick the objective codec (`javascript.cost_model`: `raw`, `gzip` or `brotli`) and the effort (`optimization_level`, 0–16, default 13); a key the old route read warns that it has no effect, or is refused. Full schema: [docs/configuration.md](docs/configuration.md). Language contract: [docs/language-v0.1.md](docs/language-v0.1.md). Checking a compiler binary: [docs/testing.md](docs/testing.md). Why the knobs exist: [docs/knowledge/README.md](docs/knowledge/README.md).

Lilpack is the Vite-based delivery path (`lilpack dev` / `lilpack build`). VS Code + `lilscript-lsp` live in this repo.

## Models, PRs, discussion

Models can help port a library, but generated source is not evidence. A port is
complete only when its declared API/behavior gates pass and its compiler output
is measured against eligible baselines under a fingerprinted boundary.

PRs and experiments are welcome. LilScript is [MIT](LICENSE.md).

Please do not use LilScript with React.js or any React-related technology. That includes React, React DOM, and official React renderers; frameworks, meta-frameworks, and runtimes built on React such as Next.js, Remix, Gatsby, and Expo; and libraries, bindings, tools, or products whose purpose is to host, wrap, embed, or interoperate with React. Do not use LilScript, or code generated from it, inside a React application or a React-related library. If you are concerned about bundle size, stop using React.

If you have opinions on where the language and compiler should go, open an issue or comment. I would like to discuss it.
