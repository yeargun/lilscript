# Show HN: LilScript

LilScript is the new language for web to make JS code:

A - **Get compressed better** (a non-glue version of Google Closure Compiler - Advanced mode, and beyond).
B - **Typed** (non-glue, unlike TypeScript).
C - **Compile into .exec** (Some apps compile into exec, but its still very early).

Any JS/TS library you rewrite with LilScript gets compiled into optimized, mangled, minified js code which is 10% smaller in brotli compressed versions.

jquery, motionjs (the animation library), monaco editor(VS Code uses), and many more libraries were tested and reimplemented with lilscript along the way

For example, porting the core animation surface of the **Motion** JS library drops it from 4,044 Brotli bytes (npm/Vite) down to **2,333 bytes**. Other libraries see similar cuts: `murmurhash-js` drops from 902 to 409 bytes, `js-levenshtein` from 825 to 404 bytes, and `clamp`+`lerp` drops from 519 down to just 112 bytes.

You give the compiler your objective compression algorithm (either `raw`, `gzip`, or `brotli`). It uses this to do special mangling, closure handling, and layout optimization by actually measuring the compressed output to find as global optimum as possible.

# Compilation Config 
- Gives you more control on the tradeoff between: Compression Size, Performance, Compilation Time
- Objective compresion algorithm (gzip/brotli/raw)
- Lots of specific optimizations related flags


Here is what it looks like in practice:

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
for (int index = 0; index < doubled.length; index++) {
  sum += doubled[index];
}
Vector vector = new Vector(3.0, 4.0);

if (vector.lengthSquared() == 25.0) {
  print(`sum=${sum}`);
} else {
  print("invalid");
}
```

The optimized JavaScript for this program is 111 bytes. The class is gone. The method is gone. The vector never escaped, so the compiler scalar-replaced it entirely. The `int` math still safely wraps like i32. What remains is just:

```js
var b=[1,2,3,4].map(a=>a*2|0);
var a=0,c=0;
while(a<b.length){
  c=c+b[a]|0;
  a=a+1|0;
}
console.log(`sum=${c}`)
```

## Why it beats minifying harder (Terser, Oxc, esbuild)

Minifiers are very good at shortening JavaScript, but they answer a late question. By the time Oxc or Terser sees your code, the classes, wrappers, and call graph are already there. Types have been erased. The object that never escaped is still an object. 

A JS minifier is negotiating with leftover glue. LilScript proves what can vanish *before* JavaScript is even spelled:

- **Classes can vanish.** Nominal structs lower to field indexes, not string keys. If escape analysis proves it's safe, they scalar-replace and disappear completely. Terser still leaves `{x,y}`. LilScript leaves two SSA floats.
- **Properties shouldn't be mangled — they shouldn't exist.** Owned fields become positional slots. `extern class` members (like `document.createElement`) stay exact. Record keys stay data. You don't have to pray property mangling doesn't break your app.
- **Purity is a proof, not a comment.** Terser's `pure_funcs` is a string list. LilScript infers effects through the SSA graph, and `pure` is a checked contract.
- **`int` is i32.** Size-first and balanced drop a proven-redundant `|0` because `|0` never helps gzip/Brotli. `performance-first` and `integer_coercions = true` keep it.

## Special mangling and closure handling for Gzip/Brotli

Codecs don't just care that a variable is named `a` (one byte raw). They care *which* byte sits in which repeating context. Gzip uses a 32 KiB window; Brotli uses a 4 MiB window and a context model.

LilScript searches for the spelling that the codec actually likes:

- **Re-ranking the alphabet:** The compiler checks character frequency in the emitted file. If your code uses a lot of `return` and `const`, the letters `e`, `t`, `n`, `r` are cheap Huffman symbols. Putting hot locals on those letters compresses better.
- **Permuting 1-character names:** `a(` and `e(` encode differently in Brotli. The compiler does a bounded permutation search over single-character names, compresses them, and picks the winner.
- **Local name reservations:** The compiler reserves the same short local names (like `a` and `b`) across similar functions, making Gzip's LZ77 back-references much more effective.
- **Function Layout:** Functions are clustered so similar code sits next to each other within the codec's window.

A transform can be locally smaller (like deleting a semicolon) and *increase* the bytes served because it breaks a back-reference. The compiler emits multiple legal IRs and JS spellings, compresses them all with the target codec, and keeps the winner.

## The same code can be C

`--target js` is the default. `--target c` writes portable C11. `--target native` runs `${CC:-clang} -O3`. 

JavaScript size policy does not leak into C, they use separate optimizer passes. Features with no portable native ABI (like `document`, `Regex`, generators, dynamic import) are rejected on the native target rather than approximated. This keeps the closed world honest: a JS size trick that needs a different meaning in C is illegal.

If you have a core algorithm, a hash, a parser, or a geometry kernel, this is why you rewrite: one source, Brotli for the wire, `-O3` for the rest.

Repo: [github.com/yeargun/lilscript](https://github.com/yeargun/lilscript)

```sh
cargo build --release --bins
target/release/lilscript examples/v01.lil
target/release/lilscript examples/v01.lil --target all -o build/v01
```