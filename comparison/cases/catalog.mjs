function add(cases, behavior, name, spec) {
  if (cases.some((entry) => entry.name === name)) {
    throw new Error(`duplicate case ${name}`);
  }
  const lilImulCalls = spec.lil.match(/\bMath\.imul\s*\(/g)?.length ?? 0;
  const jsImulCalls = spec.js.match(/\bMath\.imul\s*\(/g)?.length ?? 0;
  if (lilImulCalls !== jsImulCalls) {
    throw new Error(
      `${name}: reference Math.imul calls (${jsImulCalls}) must match explicit LilScript Math.imul calls (${lilImulCalls})`,
    );
  }
  const terserProperties = spec.terserProperties ?? true;
  const terserPropertyReason = spec.terserPropertyReason ?? null;
  if (
    !terserProperties &&
    (typeof terserPropertyReason !== "string" ||
      terserPropertyReason.trim().length === 0)
  ) {
    throw new Error(
      `${name}: a Terser property-mangling opt-out needs a non-empty reason`,
    );
  }
  if (terserProperties && terserPropertyReason !== null) {
    throw new Error(`${name}: property-mangling reasons are only for opt-outs`);
  }
  cases.push({
    name,
    behavior,
    expect: spec.expect ?? "le",
    terserProperties,
    terserPropertyReason,
    lil: spec.lil.trim() + "\n",
    js: spec.js.trim() + "\n",
  });
}

function i32Pairs() {
  return [
    [1, 2],
    [3, 5],
    [7, 9],
    [8, 13],
    [11, 17],
    [12, 19],
    [18, 4],
    [21, 6],
    [25, 3],
    [32, 8],
    [40, 2],
    [41, 1],
    [63, 7],
    [64, 16],
    [99, 11],
    [100, 25],
    [127, 3],
    [128, 4],
    [255, 5],
    [256, 16],
  ];
}

function valueLists() {
  return [
    [3, 1, 4, 1, 5, 9],
    [2, 7, 1, 8, 2, 8],
    [1, 2, 3, 4, 5, 6, 7, 8],
    [9, 8, 7, 6, 5, 4, 3, 2],
    [0, 1, 0, 2, 0, 3, 0, 4],
    [10, 20, 30, 40],
    [5, 5, 5, 5, 5, 5],
    [1, 1, 2, 3, 5, 8, 13, 21],
    [12, 4, 8, 16, 2, 6],
    [15, 3, 9, 27, 1, 81],
    [4, 9, 16, 25, 36, 49],
    [11, 22, 33, 44, 55, 66],
    [7, 14, 21, 28, 35],
    [2, 3, 5, 7, 11, 13, 17],
    [6, 1, 8, 2, 10, 3],
  ];
}

function intList(values) {
  return values.join(", ");
}

export function catalog() {
  const cases = [];

  for (const [left, right] of i32Pairs()) {
    add(cases, "constant-fold/add", `fold-add-${left}-${right}`, {
      lil: `print(${left}+${right});`,
      js: `console.log(${left}+${right});`,
    });
    add(cases, "constant-fold/subtract", `fold-sub-${left}-${right}`, {
      lil: `print(${left}-${right});`,
      js: `console.log(${left}-${right});`,
    });
    add(cases, "constant-fold/multiply", `fold-mul-${left}-${right}`, {
      lil: `print(${left}*${right});`,
      js: `console.log(${left}*${right}|0);`,
    });
  }

  const mixOps = [
    ["a-plus-twice-b", (a, b) => `${a}+${b}*2`, (a, b) => `${a}+(${b}*2|0)|0`],
    ["twice-a-minus-b", (a, b) => `${a}*2-${b}`, (a, b) => `(${a}*2|0)-${b}|0`],
    ["sum-product", (a, b) => `(${a}+${b})*${a}`, (a, b) => `((${a}+${b}|0)*${a})|0`],
  ];
  for (const [tag, lilOp, jsOp] of mixOps) {
    for (const [left, right] of i32Pairs().slice(0, 8)) {
      add(cases, `constant-fold/${tag}`, `fold-${tag}-${left}-${right}`, {
        lil: `print(${lilOp(left, right)});`,
        js: `console.log(${jsOp(left, right)});`,
      });
    }
  }

  for (const values of valueLists()) {
    const name = `loop-sum-${values.join("-")}`;
    add(cases, "loop/sum", name, {
      lil: `int[] values = [${intList(values)}];
int total = 0;
for (int i = 0; i < values.length; i++) {
  total += values[i];
}
print(total);`,
      js: `const values = [${intList(values)}];
let total = 0;
for (let i = 0; i < values.length; i++) {
  total = total + values[i] | 0;
}
console.log(total);`,
    });
  }

  for (const values of valueLists()) {
    add(cases, "loop/product-with-zero-guard", `loop-product-${values.join("-")}`, {
      lil: `int[] values = [${intList(values)}];
int total = 1;
for (int i = 0; i < values.length; i++) {
  int factor = values[i];
  if (factor == 0) {
    factor = 1;
  }
  total *= factor;
}
print(total);`,
      js: `const values = [${intList(values)}];
let total = 1;
for (let i = 0; i < values.length; i++) {
  total = total * (values[i] === 0 ? 1 : values[i]) | 0;
}
console.log(total);`,
    });
  }

  for (const values of valueLists()) {
    add(cases, "aggregate/struct-pair-layout", `struct-pairs-${values.join("-")}`, {
      expect: "lt",
      lil: `struct Point {
  int x;
  int y;
}

int score(Point point) {
  return point.x * 3 + point.y * 5;
}

int[] values = [${intList(values)}];
int total = 0;
for (int i = 0; i + 1 < values.length; i += 2) {
  total += score(Point{values[i], values[i + 1]});
}
print(total);`,
      js: `function score(point) {
  return (point.x * 3 | 0) + (point.y * 5 | 0) | 0;
}
const values = [${intList(values)}];
let total = 0;
for (let i = 0; i + 1 < values.length; i += 2) {
  total = total + score({x: values[i], y: values[i + 1]}) | 0;
}
console.log(total);`,
    });
  }

  for (const values of valueLists().slice(0, 10)) {
    add(cases, "aggregate/nested-struct-layout", `nested-struct-${values.join("-")}`, {
      expect: "lt",
      lil: `struct Point {
  int x;
  int y;
}

struct Rect {
  Point origin;
  int width;
  int height;
}

int area(Rect rect) {
  return rect.width * rect.height + rect.origin.x + rect.origin.y;
}

int[] values = [${intList(values)}];
int total = 0;
for (int i = 0; i + 3 < values.length; i += 4) {
  int w = values[i + 2];
  int h = values[i + 3];
  if (w == 0) {
    w = 1;
  }
  if (h == 0) {
    h = 1;
  }
  total += area(Rect{Point{values[i], values[i + 1]}, w, h});
}
print(total);`,
      js: `function area(rect) {
  return ((rect.width * rect.height | 0) + rect.origin.x | 0) + rect.origin.y | 0;
}
const values = [${intList(values)}];
let total = 0;
for (let i = 0; i + 3 < values.length; i += 4) {
  const w = values[i + 2] === 0 ? 1 : values[i + 2];
  const h = values[i + 3] === 0 ? 1 : values[i + 3];
  total = total + area({origin: {x: values[i], y: values[i + 1]}, width: w, height: h}) | 0;
}
console.log(total);`,
    });
  }

  for (const [start, steps] of [
    [0, 8],
    [3, 10],
    [5, 12],
    [7, 9],
    [11, 11],
    [2, 15],
    [13, 8],
    [4, 14],
    [9, 13],
    [1, 16],
    [8, 7],
    [6, 18],
  ]) {
    add(cases, "aggregate/class-counter-mutation", `class-counter-${start}-${steps}`, {
      expect: "lt",
      lil: `class Counter {
  int value;

  init(int initial) {
    this.value = initial;
  }

  int add(int amount) {
    this.value += amount;
    return this.value;
  }
}

Counter counter = new Counter(${start});
int total = 0;
for (int i = 1; i <= ${steps}; i++) {
  total += counter.add(i);
}
print(total);
print(counter.value);`,
      js: `class Counter {
  constructor(initial) {
    this.value = initial;
  }
  add(amount) {
    this.value = this.value + amount | 0;
    return this.value;
  }
}
const counter = new Counter(${start});
let total = 0;
for (let i = 1; i <= ${steps}; i++) {
  total = total + counter.add(i) | 0;
}
console.log(total);
console.log(counter.value);`,
    });
  }

  for (const n of [4, 5, 6, 7, 8, 9, 10, 11, 12]) {
    add(cases, "loop/nested-score", `nested-loop-score-${n}`, {
      lil: `int score(int limit) {
  int total = 0;
  for (int outer = 0; outer < limit; outer++) {
    if (outer % 3 == 0) {
      continue;
    }
    int inner = 0;
    while (inner < 4) {
      if ((outer + inner) % 2 == 0) {
        total += outer * inner;
      } else {
        total += 1;
      }
      inner++;
    }
  }
  return total;
}

print(score(${n}));`,
      js: `function score(limit) {
  let total = 0;
  for (let outer = 0; outer < limit; outer++) {
    if (outer % 3 === 0) continue;
    let inner = 0;
    while (inner < 4) {
      if ((outer + inner) % 2 === 0) {
        total = total + (outer * inner | 0) | 0;
      } else {
        total = total + 1 | 0;
      }
      inner++;
    }
  }
  return total;
}
console.log(score(${n}));`,
    });
  }

  for (const n of [5, 6, 7, 8, 9, 10]) {
    add(cases, "function/recursive-factorial", `factorial-${n}`, {
      lil: `int factorial(int value) {
  if (value <= 1) {
    return 1;
  }
  return value * factorial(value - 1);
}
print(factorial(${n}));`,
      js: `function factorial(value) {
  if (value <= 1) return 1;
  return value * factorial(value - 1 | 0) | 0;
}
console.log(factorial(${n}));`,
    });
  }

  const gcdPairs = [
    [1071, 462],
    [48, 18],
    [270, 192],
    [100, 35],
    [144, 89],
    [81, 27],
    [99, 27],
    [512, 48],
  ];
  for (const [left, right] of gcdPairs) {
    add(cases, "loop/euclidean-gcd", `gcd-${left}-${right}`, {
      lil: `int gcd(int left, int right) {
  while (right != 0) {
    int remainder = left % right;
    left = right;
    right = remainder;
  }
  return left;
}
print(gcd(${left}, ${right}));`,
      js: `function gcd(left, right) {
  while (right !== 0) {
    const remainder = left % right | 0;
    left = right;
    right = remainder;
  }
  return left;
}
console.log(gcd(${left}, ${right}));`,
    });
  }

  for (const n of [8, 10, 12, 14, 16, 18]) {
    add(cases, "function/iterative-fibonacci", `fibonacci-${n}`, {
      lil: `int fibonacci(int count) {
  int previous = 0;
  int current = 1;
  for (int index = 0; index < count; index++) {
    int next = previous + current;
    previous = current;
    current = next;
  }
  return previous;
}
print(fibonacci(${n}));`,
      js: `function fibonacci(count) {
  let previous = 0;
  let current = 1;
  for (let index = 0; index < count; index++) {
    const next = previous + current | 0;
    previous = current;
    current = next;
  }
  return previous;
}
console.log(fibonacci(${n}));`,
    });
  }

  const words = [
    ["alpha", "beta"],
    ["lilscript", "javascript"],
    ["compress", "brotli"],
    ["mangle", "identifier"],
    ["closed", "world"],
    ["typed", "aggregate"],
    ["host", "alias"],
    ["scalar", "replace"],
    ["dead", "code"],
    ["inline", "call"],
    ["struct", "layout"],
    ["codec", "bytes"],
  ];
  for (const [left, right] of words) {
    add(cases, "string/constant-concatenation", `string-concat-${left}-${right}`, {
      lil: `string left = "${left}";
string right = "${right}";
print(left + "-" + right);
print(left.length + right.length);`,
      js: `const left = "${left}";
const right = "${right}";
console.log(left + "-" + right);
console.log(left.length + right.length);`,
    });
  }

  for (const [word, needle] of [
    ["LilScript", "Lil"],
    ["LilScript", "Script"],
    ["compression", "press"],
    ["javascript", "script"],
    ["hasOwnProperty", "Own"],
    ["ArrayPrototype", "Proto"],
    ["functionToString", "To"],
    ["objectHasOwn", "Has"],
  ]) {
    add(cases, "string/search-predicates", `string-search-${word}-${needle}`, {
      lil: `string word = "${word}";
if (word.includes("${needle}")) {
  print(1);
} else {
  print(0);
}
if (word.startsWith("${needle}")) {
  print(1);
} else {
  print(0);
}
if (word.endsWith("${needle}")) {
  print(1);
} else {
  print(0);
}`,
      js: `const word = "${word}";
console.log(word.includes("${needle}") ? 1 : 0);
console.log(word.startsWith("${needle}") ? 1 : 0);
console.log(word.endsWith("${needle}") ? 1 : 0);`,
    });
  }

  for (const values of valueLists().slice(0, 8)) {
    add(cases, "collection/array-pipeline", `pipeline-${values.join("-")}`, {
      expect: "lt",
      lil: `int factor = 3;
int[] values = [${intList(values)}];
auto scaled = values.map((int value) => value * factor);
auto selected = scaled.filter((int value) => value % 2 == 0);
int total = selected.reduce((int sum, int value) => sum + value, 0);
print(total);
print(selected.length);`,
      js: `const factor = 3;
const values = [${intList(values)}];
const scaled = values.map((value) => value * factor | 0);
const selected = scaled.filter((value) => (value % 2 | 0) === 0);
const total = selected.reduce((sum, value) => sum + value | 0, 0);
console.log(total);
console.log(selected.length);`,
    });
  }

  for (const n of [3, 4, 5, 6, 7, 8]) {
    add(cases, "function/dead-helper-elimination", `dce-unused-${n}`, {
      lil: `int live(int value) {
  return value * ${n} + 1;
}
int unused(int value) {
  return value * 1000 + 42;
}
print(live(${n}));`,
      js: `function live(value) {
  return (value * ${n} | 0) + 1 | 0;
}
function unused(value) {
  return (value * 1000 | 0) + 42 | 0;
}
console.log(live(${n}));`,
    });
  }

  for (const n of [2, 3, 4, 5, 6, 7, 8, 9]) {
    add(cases, "function/identical-helper-folding", `identical-helpers-${n}`, {
      lil: `int doubleA(int value) {
  return value + value;
}
int doubleB(int value) {
  return value + value;
}
print(doubleA(${n}));
print(doubleB(${n + 3}));`,
      js: `function doubleA(value) {
  return value + value | 0;
}
function doubleB(value) {
  return value + value | 0;
}
console.log(doubleA(${n}));
console.log(doubleB(${n + 3}));`,
    });
  }

  for (const n of [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]) {
    add(cases, "control/if-chain", `if-chain-${n}`, {
      lil: `int classify(int value) {
  if (value < 3) {
    return 1;
  }
  if (value < 6) {
    return 2;
  }
  if (value < 9) {
    return 3;
  }
  return 4;
}
print(classify(${n}));`,
      js: `function classify(value) {
  if (value < 3) return 1;
  if (value < 6) return 2;
  if (value < 9) return 3;
  return 4;
}
console.log(classify(${n}));`,
    });
  }

  for (const [left, right] of i32Pairs().slice(0, 12)) {
    add(cases, "integer/bitwise-operations", `bitops-${left}-${right}`, {
      lil: `print(${left}&${right});
print(${left}|${right});
print(${left}^${right});
print(${left}<<1);
print(${right}>>1);`,
      js: `console.log(${left}&${right});
console.log(${left}|${right});
console.log(${left}^${right});
console.log(${left}<<1);
console.log(${right}>>1);`,
    });
  }

  for (const n of [2, 3, 4, 5, 6, 7, 8]) {
    add(cases, "closure/capture", `closure-capture-${n}`, {
      lil: `int run() {
  int factor = ${n};
func(int)->int scale = (int value) => value * factor;
int total = 0;
for (int i = 1; i <= 6; i++) {
  total += scale(i);
}
  return total;
}
print(run());`,
      js: `function run() {
  const factor = ${n};
const scale = (value) => value * factor | 0;
let total = 0;
for (let i = 1; i <= 6; i++) {
  total = total + scale(i) | 0;
}
  return total;
}
console.log(run());`,
    });
  }

  for (const n of [0, 1, 2, 3, 4, 5, 6, 7]) {
    add(cases, "boolean/logical-folding", `bool-logic-${n}`, {
      lil: `bool a = ${n} % 2 == 0;
bool b = ${n} > 3;
if (a && b) {
  print(1);
} else {
  print(0);
}
if (a || b) {
  print(1);
} else {
  print(0);
}
if (!a) {
  print(1);
} else {
  print(0);
}`,
      js: `const a = ${n} % 2 === 0;
const b = ${n} > 3;
console.log(a && b ? 1 : 0);
console.log(a || b ? 1 : 0);
console.log(!a ? 1 : 0);`,
    });
  }

  for (const n of [0, 1, 2, 3, 4, 5, 6, 7, 8]) {
    add(cases, "control/early-return", `early-return-${n}`, {
      lil: `int firstPositive(int[] values) {
  for (int i = 0; i < values.length; i++) {
    if (values[i] > ${n}) {
      return values[i];
    }
  }
  return -1;
}
print(firstPositive([1, 3, 5, 7, 9]));`,
      js: `function firstPositive(values) {
  for (let i = 0; i < values.length; i++) {
    if ((values[i] | 0) > ${n}) return values[i] | 0;
  }
  return -1;
}
console.log(firstPositive([1, 3, 5, 7, 9]));`,
    });
  }

  for (const n of [3, 4, 5, 6, 7]) {
    add(cases, "control/dead-branch", `dead-branch-${n}`, {
      lil: `int live = ${n} * 2;
print(live);
if (false) {
  print(live * 100);
}`,
      js: `const live = ${n} * 2 | 0;
console.log(live);
if (false) {
  console.log(live * 100 | 0);
}`,
    });
  }

  const labels = [
    "application-build-identifier",
    "compression-first-language-label",
    "closed-world-mangling-token",
  ];
  for (const label of labels) {
    add(cases, "string/pooling", `string-pool-${label}`, {
      expect: "lt",
      lil: `print("${label}");
print("${label}");
print("${label}");
print("${label}");`,
      js: `console.log("${label}");
console.log("${label}");
console.log("${label}");
console.log("${label}");`,
    });
  }

  for (const n of [4, 6, 8, 10, 12]) {
    add(cases, "control/integer-tag-dispatch", `enum-int-dispatch-${n}`, {
      expect: "lt",
      lil: `int dispatch(int kind, int value) {
  if (kind == 0) {
    return value;
  }
  if (kind == 1) {
    return value * 2;
  }
  if (kind == 2) {
    return value * 3 + 1;
  }
  return value - 1;
}
int total = 0;
for (int i = 0; i < ${n}; i++) {
  total += dispatch(i % 4, i + 3);
}
print(total);`,
      js: `function dispatch(kind, value) {
  if (kind === 0) return value;
  if (kind === 1) return value * 2 | 0;
  if (kind === 2) return (value * 3 | 0) + 1 | 0;
  return value - 1 | 0;
}
let total = 0;
for (let i = 0; i < ${n}; i++) {
  total = total + dispatch(i % 4 | 0, i + 3) | 0;
}
console.log(total);`,
    });
  }

  for (const values of valueLists().slice(0, 6)) {
    add(cases, "aggregate/fixed-matrix", `matrix2-${values.join("-")}`, {
      expect: "lt",
      lil: `struct Mat2 {
  int a;
  int b;
  int c;
  int d;
}

int det(Mat2 m) {
  return m.a * m.d - m.b * m.c;
}

int[] values = [${intList(values)}];
int total = 0;
for (int i = 0; i + 3 < values.length; i += 4) {
  total += det(Mat2{values[i], values[i + 1], values[i + 2], values[i + 3]});
}
print(total);`,
      js: `function det(m) {
  return (m.a * m.d | 0) - (m.b * m.c | 0) | 0;
}
const values = [${intList(values)}];
let total = 0;
for (let i = 0; i + 3 < values.length; i += 4) {
  total = total + det({a: values[i], b: values[i + 1], c: values[i + 2], d: values[i + 3]}) | 0;
}
console.log(total);`,
    });
  }

  for (const n of [5, 6, 7, 8, 9, 10, 12, 14]) {
    add(cases, "host/math-max", `host-math-max-${n}`, {
      lil: `extern float mathMax(float a, float b);
extern float mathMin(float a, float b);
float hi = 0.0;
float lo = 100.0;
for (int i = 1; i <= ${n}; i++) {
  float v = i;
  hi = mathMax(hi, v);
  lo = mathMin(lo, v);
}
print(hi);
print(lo);`,
      js: `let hi = 0;
let lo = 100;
for (let i = 1; i <= ${n}; i++) {
  hi = Math.max(hi, i);
  lo = Math.min(lo, i);
}
console.log(hi);
console.log(lo);`,
    });
  }

  for (const keys of [
    ["a", "b", "c"],
    ["name", "id", "ok"],
    ["x", "y"],
    ["left", "right", "mid"],
    ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta"],
  ]) {
    const sets = keys
      .map((key, index) => `JS.set(object, "${key}", ${index + 1});`)
      .join("\n");
    const jsObj = Object.fromEntries(keys.map((key, index) => [key, index + 1]));
    const checks = keys
      .map((key) => `if (objectHasOwn(object, "${key}")) {
  print(1);
} else {
  print(0);
}`)
      .join("\n");
    const jsChecks = keys
      .map((key) => `if (Object.hasOwn(object, "${key}")) {
  console.log(1);
} else {
  console.log(0);
}`)
      .join("\n");
    add(cases, "host/object-has-own", `host-hasown-${keys.join("-")}`, {
      lil: `extern bool objectHasOwn(JsValue obj, string key);
JsValue object = JS.object();
${sets}
if (objectHasOwn(object, "missing")) {
  print(1);
} else {
  print(0);
}
${checks}`,
      js: `const object = ${JSON.stringify(jsObj)};
if (Object.hasOwn(object, "missing")) {
  console.log(1);
} else {
  console.log(0);
}
${jsChecks}`,
    });
  }

  add(cases, "host/object-has-own-detached", `host-hasown-as-value`, {
    terserProperties: false,
    terserPropertyReason:
      "Object.hasOwn receives quoted public keys whose spelling is observable",
    lil: `extern bool objectHasOwn(JsValue obj, string key);
JsValue object = JS.object();
JS.set(object, "keep", 1);
func(JsValue, string)->bool has = objectHasOwn;
if (has(object, "keep")) {
  print(1);
} else {
  print(0);
}
if (has(object, "gone")) {
  print(1);
} else {
  print(0);
}`,
    js: `const object = {keep: 1};
const has = Object.hasOwn;
if (has(object, "keep")) {
  console.log(1);
} else {
  console.log(0);
}
if (has(object, "gone")) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  for (const n of [3, 4, 5, 6, 7, 8]) {
    add(cases, "function/default-arguments", `default-extra-args-${n}`, {
      lil: `int add(int left, int right = 1, int extra = 0) {
  return left + right + extra;
}
print(add(${n}));
print(add(${n}, 2));
print(add(${n}, 2, 3));`,
      js: `function add(left, right, extra) {
  if (right === void 0) right = 1;
  if (extra === void 0) extra = 0;
  return left + right + extra | 0;
}
console.log(add(${n}));
console.log(add(${n}, 2));
console.log(add(${n}, 2, 3));`,
    });
  }

  for (const values of valueLists().slice(0, 8)) {
    add(cases, "collection/minmax-scan", `minmax-scan-${values.join("-")}`, {
      lil: `int[] values = [${intList(values)}];
int lo = values[0];
int hi = values[0];
for (int i = 1; i < values.length; i++) {
  int value = values[i];
  if (value < lo) {
    lo = value;
  }
  if (value > hi) {
    hi = value;
  }
}
print(lo);
print(hi);`,
      js: `const values = [${intList(values)}];
let lo = values[0];
let hi = values[0];
for (let i = 1; i < values.length; i++) {
  const value = values[i];
  if (value < lo) lo = value;
  if (value > hi) hi = value;
}
console.log(lo);
console.log(hi);`,
    });
  }

  for (const n of [4, 5, 6, 7, 8, 9, 10]) {
    add(cases, "loop/while-countdown", `while-countdown-${n}`, {
      lil: `int value = ${n};
int total = 0;
while (value > 0) {
  total += value;
  value--;
}
print(total);`,
      js: `let value = ${n};
let total = 0;
while (value > 0) {
  total = total + value | 0;
  value = value - 1 | 0;
}
console.log(total);`,
    });
  }

  for (const n of [2, 3, 4, 5, 6]) {
    add(cases, "function/helper-composition", `nested-fn-local-${n}`, {
      lil: `int inner(int x, int value) {
  return x * ${n} + value;
}
int outer(int value) {
  return inner(value, value) + inner(value + 1, value);
}
print(outer(${n}));`,
      js: `function inner(x, value) {
  return (x * ${n} | 0) + value | 0;
}
function outer(value) {
  return inner(value, value) + inner(value + 1, value) | 0;
}
console.log(outer(${n}));`,
    });
  }

  add(cases, "winner/aggregate-model", `win-aggregate-model`, {
    expect: "lt",
    lil: `struct Point {
  int x;
  int y;
}

struct Rectangle {
  Point origin;
  int width;
  int height;
}

int area(Rectangle rectangle) {
  return rectangle.width * rectangle.height;
}

class ModelCounter {
  int value;

  init(int initial) {
    this.value = initial;
  }

  int add(int amount) {
    this.value += amount;
    return this.value;
  }
}

Rectangle rectangle = Rectangle{Point{3, 4}, 6, 7};
ModelCounter counter = new ModelCounter(rectangle.origin.x + rectangle.origin.y);
print(area(rectangle));
print(counter.add(rectangle.width));
print(counter.add(rectangle.height));`,
    js: `function area(rectangle) {
  return rectangle.width * rectangle.height | 0;
}
class ModelCounter {
  constructor(initial) {
    this.value = initial;
  }
  add(amount) {
    this.value = this.value + amount | 0;
    return this.value;
  }
}
const rectangle = {origin: {x: 3, y: 4}, width: 6, height: 7};
const counter = new ModelCounter(rectangle.origin.x + rectangle.origin.y | 0);
console.log(area(rectangle));
console.log(counter.add(rectangle.width));
console.log(counter.add(rectangle.height));`,
  });

  add(cases, "winner/optimizer-pressure", `win-optimizer-pressure`, {
    expect: "lt",
    lil: `int factor = 6;
int increment(int value) {
  return value + 1;
}
int threeSteps(int value) {
  return increment(increment(increment(value)));
}
int repeated(int value) {
  int first = value * 7;
  int second = 7 * value;
  return first + second;
}
int unused(int value) {
  return value * 1000;
}
class Box {
  int value;
  init(int value) {
    this.value = value;
  }
  int plus(int amount) {
    return this.value + amount;
  }
}
Box box = new Box(40);
print(threeSteps(1));
print(repeated(factor));
print(box.plus(2));
print("application-build-identifier");
print("application-build-identifier");
print("application-build-identifier");
if (false) {
  print(unused(9));
}`,
    js: `const factor = 6;
function increment(value) {
  return value + 1 | 0;
}
function threeSteps(value) {
  return increment(increment(increment(value)));
}
function repeated(value) {
  const first = value * 7 | 0;
  const second = 7 * value | 0;
  return first + second | 0;
}
function unused(value) {
  return value * 1000 | 0;
}
class Box {
  constructor(value) {
    this.value = value;
  }
  plus(amount) {
    return this.value + amount | 0;
  }
}
const box = new Box(40);
console.log(threeSteps(1));
console.log(repeated(factor));
console.log(box.plus(2));
console.log("application-build-identifier");
console.log("application-build-identifier");
console.log("application-build-identifier");
if (false) {
  console.log(unused(9));
}`,
  });

  add(cases, "loop/nested-score", `win-control-flow`, {
    expect: "lt",
    lil: `int score(int limit) {
  int total = 0;
  for (int outer = 0; outer < limit; outer++) {
    if (outer % 3 == 0) {
      continue;
    }
    int inner = 0;
    while (inner < 4) {
      if ((outer + inner) % 2 == 0) {
        total += outer * inner;
      } else {
        total += 1;
      }
      inner++;
    }
  }
  return total;
}
print(score(12));`,
    js: `function score(limit) {
  let total = 0;
  for (let outer = 0; outer < limit; outer++) {
    if (outer % 3 === 0) continue;
    let inner = 0;
    while (inner < 4) {
      if ((outer + inner) % 2 === 0) {
        total = total + (outer * inner | 0) | 0;
      } else {
        total = total + 1 | 0;
      }
      inner++;
    }
  }
  return total;
}
console.log(score(12));`,
  });

  for (const [lo, hi, value] of [
    [0, 10, 3],
    [0, 10, 11],
    [0, 10, -2],
    [5, 20, 12],
    [5, 20, 4],
    [5, 20, 25],
    [-3, 3, 0],
    [-3, 3, 8],
    [1, 100, 50],
    [1, 100, 0],
  ]) {
    add(cases, "number/clamp", `clamp-${lo}-${hi}-${value}`.replaceAll("-", "n"), {
      lil: `int clamp(int value, int lo, int hi) {
  if (value < lo) {
    return lo;
  }
  if (value > hi) {
    return hi;
  }
  return value;
}
print(clamp(${value}, ${lo}, ${hi}));`,
      js: `function clamp(value, lo, hi) {
  if (value < lo) return lo;
  if (value > hi) return hi;
  return value;
}
console.log(clamp(${value}, ${lo}, ${hi}));`,
    });
  }

  for (const values of valueLists().slice(0, 8)) {
    add(cases, "collection/prefix-sum", `prefix-sum-${values.join("-")}`, {
      lil: `int[] values = [${intList(values)}];
int running = 0;
for (int i = 0; i < values.length; i++) {
  running += values[i];
  print(running);
}`,
      js: `const values = [${intList(values)}];
let running = 0;
for (let i = 0; i < values.length; i++) {
  running = running + values[i] | 0;
  console.log(running);
}`,
    });
  }

  for (const n of [3, 4, 5, 6, 7, 8, 9, 10]) {
    add(cases, "loop/triangle-sum", `triangle-sum-${n}`, {
      lil: `int total = 0;
for (int i = 1; i <= ${n}; i++) {
  for (int j = 1; j <= i; j++) {
    total += i * j;
  }
}
print(total);`,
      js: `let total = 0;
for (let i = 1; i <= ${n}; i++) {
  for (let j = 1; j <= i; j++) {
    total = total + (i * j | 0) | 0;
  }
}
console.log(total);`,
    });
  }

  for (const word of ["compress", "lilscript", "javascript", "brotli", "mangling", "aggregate"]) {
    add(cases, "string/character-iteration", `string-chars-${word}`, {
      lil: `string word = "${word}";
int total = 0;
for (int i = 0; i < word.length; i++) {
  total += word.charCodeAt(i);
}
print(total);
print(word.toLowerCase());
print(word.toUpperCase());`,
      js: `const word = "${word}";
let total = 0;
for (let i = 0; i < word.length; i++) {
  total = total + word.charCodeAt(i) | 0;
}
console.log(total);
console.log(word.toLowerCase());
console.log(word.toUpperCase());`,
    });
  }

  for (const n of [2, 3, 4, 5, 6, 7]) {
    add(cases, "aggregate/struct-property-transform", `record-transform-${n}`, {
      expect: "lt",
      lil: `struct Item {
  int id;
  int weight;
}

int score(Item item) {
  return item.id * 10 + item.weight;
}

int total = 0;
for (int i = 0; i < ${n}; i++) {
  total += score(Item{i + 1, i * 3 + 2});
}
print(total);`,
      js: `function score(item) {
  return (item.id * 10 | 0) + item.weight | 0;
}
let total = 0;
for (let i = 0; i < ${n}; i++) {
  total = total + score({id: i + 1, weight: (i * 3 | 0) + 2 | 0}) | 0;
}
console.log(total);`,
    });
  }

  for (const n of [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 24, 25, 30, 32, 36]) {
    add(cases, "integer/modulo-bucket", `mod-bucket-${n}`, {
      lil: `int total = 0;
for (int i = 0; i < ${n}; i++) {
  int bucket = i % 4;
  if (bucket == 0) {
    total += i;
  } else if (bucket == 1) {
    total += i * 2;
  } else if (bucket == 2) {
    total += i * 3;
  } else {
    total -= 1;
  }
}
print(total);`,
      js: `let total = 0;
for (let i = 0; i < ${n}; i++) {
  const bucket = i % 4 | 0;
  if (bucket === 0) total = total + i | 0;
  else if (bucket === 1) total = total + (i * 2 | 0) | 0;
  else if (bucket === 2) total = total + (i * 3 | 0) | 0;
  else total = total - 1 | 0;
}
console.log(total);`,
    });
  }

  for (const word of ["Ab", "CdEf", "HiJkLm", "NoPqRsTu", "VwXyZ", "Script", "Encode", "Decode"]) {
    add(cases, "string/index-access", `string-index-${word}`, {
      lil: `string word = "${word}";
print(word.indexOf("a"));
print(word.indexOf("A"));
print(word.toLowerCase().indexOf("a"));`,
      js: `const word = "${word}";
console.log(word.indexOf("a"));
console.log(word.indexOf("A"));
console.log(word.toLowerCase().indexOf("a"));`,
    });
  }

  add(cases, "winner/point-batch", `win-point-batch`, {
    expect: "lt",
    lil: `struct Point {
  int x;
  int y;
}

int manhattan(Point point) {
  int x = point.x;
  if (x < 0) {
    x = -x;
  }
  int y = point.y;
  if (y < 0) {
    y = -y;
  }
  return x + y;
}

int[] values = [3, -4, 5, 6, -7, 8, 1, -2];
int total = 0;
for (int i = 0; i + 1 < values.length; i += 2) {
  total += manhattan(Point{values[i], values[i + 1]});
}
print(total);`,
    js: `function manhattan(point) {
  let x = point.x;
  if (x < 0) x = -x | 0;
  let y = point.y;
  if (y < 0) y = -y | 0;
  return x + y | 0;
}
const values = [3, -4, 5, 6, -7, 8, 1, -2];
let total = 0;
for (let i = 0; i + 1 < values.length; i += 2) {
  total = total + manhattan({x: values[i], y: values[i + 1]}) | 0;
}
console.log(total);`,
  });

  add(cases, "winner/class-scale", `win-class-scale`, {
    expect: "lt",
    lil: `class Scale {
  int factor;

  init(int factor) {
    this.factor = factor;
  }

  int apply(int value) {
    return value * this.factor;
  }
}

Scale scale = new Scale(3);
int total = 0;
for (int i = 1; i <= 8; i++) {
  total += scale.apply(i);
}
print(total);
print(scale.factor);`,
    js: `class Scale {
  constructor(factor) {
    this.factor = factor;
  }
  apply(value) {
    return value * this.factor | 0;
  }
}
const scale = new Scale(3);
let total = 0;
for (let i = 1; i <= 8; i++) {
  total = total + scale.apply(i) | 0;
}
console.log(total);
console.log(scale.factor);`,
  });

  // Dynamic arithmetic cases deliberately pass values through calls and loops. They
  // cover the i32 rules that differ from JavaScript's default number arithmetic.
  for (const [seed, factor, offset, rounds, divisor] of [
    [2147483647, 3, 1, 2, 7],
    [-2147483647, 5, -9, 3, -3],
    [1073741824, 7, 13, 4, 11],
    [-1073741824, -3, 17, 2, 5],
    [65535, 65537, 23, 3, 97],
    [-65536, 32769, -31, 4, -17],
    [123456789, 1664525, 1013904223, 3, 257],
    [-987654321, 1103515245, 12345, 2, 4093],
    [1, -2147483647, -1, 3, 19],
    [-1, 2147483647, 1, 3, -23],
  ]) {
    const tag = `${seed}-${factor}-${offset}-${rounds}-${divisor}`.replaceAll("-", "n");
    add(cases, "integer/i32-overflow-churn", `edge-i32-churn-${tag}`, {
      lil: `int churn(int value, int factor, int offset, int rounds) {
  for (int i = 0; i < rounds; i++) {
    value = value * factor + offset;
  }
  return value;
}
int value = churn(${seed}, ${factor}, ${offset}, ${rounds});
print(value);
print(value / ${divisor});
print(value % ${divisor});
print(value >>> 5);`,
      js: `function churn(value, factor, offset, rounds) {
  for (let i = 0; i < rounds; i++) {
    value = (value * factor | 0) + offset | 0;
  }
  return value;
}
const value = churn(${seed}, ${factor}, ${offset}, ${rounds});
console.log(value);
console.log(value / ${divisor} | 0);
console.log(value % ${divisor} | 0);
console.log(value >>> 5);`,
    });
  }

  for (const [start, delta, scale, rounds] of [
    [0.25, 0.5, 1.5, 3],
    [-0.5, 1.25, 2.0, 4],
    [1.125, -0.25, 3.5, 5],
    [10.0, -1.5, 0.5, 4],
    [3.75, 0.125, -2.0, 3],
    [-8.25, 2.5, -0.25, 5],
    [100.5, -10.25, 0.125, 4],
    [0.0625, 0.03125, 16.0, 6],
    [-1.0, -0.75, 4.0, 3],
    [7.5, 1.5, 0.25, 7],
  ]) {
    const tag = `${start}-${delta}-${scale}-${rounds}`.replaceAll("-", "n").replaceAll(".", "p");
    add(cases, "number/fractional-flow", `edge-number-flow-${tag}`, {
      lil: `number evolve(number value, number delta, number scale, int rounds) {
  for (int i = 0; i < rounds; i++) {
    value = (value + delta) * scale;
  }
  return value;
}
number value = evolve(${start}, ${delta}, ${scale}, ${rounds});
print(value);
print(value > 0.0);
print(value == evolve(${start}, ${delta}, ${scale}, ${rounds}));`,
      js: `function evolve(value, delta, scale, rounds) {
  for (let i = 0; i < rounds; i++) value = (value + delta) * scale;
  return value;
}
const value = evolve(${start}, ${delta}, ${scale}, ${rounds});
console.log(value);
console.log(value > 0);
console.log(value === evolve(${start}, ${delta}, ${scale}, ${rounds}));`,
    });
  }

  for (const [first, second, third] of [
    [0, 2, 4], [1, 2, 4], [2, 3, 5], [3, 4, 6], [4, 5, 8],
    [5, 7, 9], [6, 8, 10], [7, 9, 12], [8, 11, 14], [9, 12, 16],
  ]) {
    add(cases, "control/short-circuit-effects", `edge-short-circuit-${first}-${second}-${third}`, {
      lil: `int calls = 0;
bool probe(int value) {
  calls += value;
  return value % 2 == 0;
}
bool result = probe(${first}) && probe(${second}) || probe(${third});
print(result);
print(calls);`,
      js: `let calls = 0;
function probe(value) {
  calls = calls + value | 0;
  return value % 2 === 0;
}
const result = probe(${first}) && probe(${second}) || probe(${third});
console.log(result);
console.log(calls);`,
    });
  }

  for (const [limit, skip, stop] of [
    [8, 2, 7], [9, 3, 8], [10, 4, 9], [11, 5, 10], [12, 2, 9],
    [13, 3, 11], [14, 4, 12], [15, 5, 13], [16, 6, 14], [18, 7, 15],
  ]) {
    add(cases, "loop/break-continue", `edge-loop-control-${limit}-${skip}-${stop}`, {
      lil: `int scan(int limit, int skip, int stop) {
  int total = 0;
  for (int i = 0; i < limit; i++) {
    if (i == skip) {
      continue;
    }
    if (i == stop) {
      break;
    }
    int j = 0;
    while (j < 3) {
      total += i + j;
      j++;
    }
  }
  return total;
}
print(scan(${limit}, ${skip}, ${stop}));`,
      js: `function scan(limit, skip, stop) {
  let total = 0;
  for (let i = 0; i < limit; i++) {
    if (i === skip) continue;
    if (i === stop) break;
    let j = 0;
    while (j < 3) {
      total = total + i + j | 0;
      j++;
    }
  }
  return total;
}
console.log(scan(${limit}, ${skip}, ${stop}));`,
    });
  }

  for (const [seed, step, count] of [
    [1, 2, 4], [3, -1, 5], [5, 3, 6], [7, -2, 7], [11, 4, 5],
    [-4, 5, 6], [20, -3, 4], [2, 7, 5], [9, 1, 8], [-8, -2, 6],
  ]) {
    const tag = `${seed}-${step}-${count}`.replaceAll("-", "n");
    add(cases, "closure/mutable-capture", `edge-closure-mutation-${tag}`, {
      lil: `void run() {
  int current = ${seed};
int calls = 0;
func(int)->int advance = (int amount) => {
  calls++;
  current += amount;
  return current;
};
int total = 0;
for (int i = 0; i < ${count}; i++) {
  total += advance(${step} + i);
}
print(total);
print(current);
  print(calls);
}
run();`,
      js: `function run() {
  let current = ${seed};
let calls = 0;
const advance = amount => {
  calls++;
  current = current + amount | 0;
  return current;
};
let total = 0;
for (let i = 0; i < ${count}; i++) total = total + advance(${step} + i | 0) | 0;
console.log(total);
console.log(current);
  console.log(calls);
}
run();`,
    });
  }

  const arrayCases = [
    ["push-pop", "print(alias.push(9)); print(values.pop());", "console.log(alias.push(9)); console.log(values.pop());"],
    ["slice-negative", "int[] copy = alias.slice(-3, -1); copy[0] = 20; print(copy.join(\"-\")); print(values.join(\"-\"));", "const copy = alias.slice(-3, -1); copy[0] = 20; console.log(copy.join(\"-\")); console.log(values.join(\"-\"));"],
    ["copy-within", "alias.copyWithin(1, 0, 3); print(values.join(\"-\"));", "alias.copyWithin(1, 0, 3); console.log(values.join(\"-\"));"],
    ["reverse", "alias.reverse(); print(values.join(\"-\"));", "alias.reverse(); console.log(values.join(\"-\"));"],
    ["fill", "int[] same = alias.fill(7); print(same == values); print(values.join(\"-\"));", "const same = alias.fill(7); console.log(same === values); console.log(values.join(\"-\"));"],
    ["concat", "int[] combined = alias.concat([8, 9]); print(combined.join(\"-\")); print(values.length);", "const combined = alias.concat([8, 9]); console.log(combined.join(\"-\")); console.log(values.length);"],
    ["some-every", "print(alias.some((int value) => value > 3)); print(alias.every((int value) => value > 0));", "console.log(alias.some(value => value > 3)); console.log(alias.every(value => value > 0));"],
    ["find-index", "print(alias.findIndex((int value) => value % 3 == 0)); print(values.indexOf(99));", "console.log(alias.findIndex(value => value % 3 === 0)); console.log(values.indexOf(99));"],
    ["pipeline", "int[] mapped = alias.map((int value) => value * 2); int[] selected = mapped.filter((int value) => value % 4 == 0); print(selected.reduce((int total, int value) => total + value, 0));", "const mapped = alias.map(value => value * 2 | 0); const selected = mapped.filter(value => value % 4 === 0); console.log(selected.reduce((total, value) => total + value | 0, 0));"],
    ["snapshot", "int seen = 0; alias.forEach((int value) => { seen += value; if (value == 1) { alias.push(10); } }); print(seen); print(values.length);", "let seen = 0; alias.forEach(value => { seen = seen + value | 0; if (value === 1) alias.push(10); }); console.log(seen); console.log(values.length);"],
  ];
  for (const [tag, lilBody, jsBody] of arrayCases) {
    add(cases, `collection/array-${tag}`, `edge-array-${tag}`, {
      lil: `int[] values = [1, 2, 3, 4, 5];
int[] alias = values;
print(alias == values);
${lilBody}`,
      js: `const values = [1, 2, 3, 4, 5];
const alias = values;
console.log(alias === values);
${jsBody}`,
    });
  }

  for (const [seed, removed, missing] of [
    [1, "left", "absent"], [2, "right", "none"], [3, "middle", "missing"],
    [4, "alpha", "void"], [5, "beta", "unknown"], [6, "gamma", "gone"],
    [7, "delta", "lost"], [8, "first", "last"], [9, "hot", "cold"], [10, "live", "dead"],
  ]) {
    add(cases, "record/json-enumeration", `edge-record-json-${seed}-${removed}`, {
      terserProperties: false,
      terserPropertyReason:
        "Object.keys and JSON.stringify expose the record's public key spellings",
      lil: `Record<int> source = record{left:${seed}, right:${seed + 1}, middle:${seed + 2}};
Record<int> copy = record{...source, right:${seed + 10}};
source.left = ${seed + 20};
print(copy.left ?? 0);
print(copy.right ?? 0);
print(copy.${missing} ?? -1);
print(Object.keys(copy).join(","));
print(JSON.stringify(copy));`,
      js: `const source = {left:${seed}, right:${seed + 1}, middle:${seed + 2}};
const copy = {...source, right:${seed + 10}};
source.left = ${seed + 20};
console.log(copy.left ?? 0);
console.log(copy.right ?? 0);
console.log(copy.${missing} ?? -1);
console.log(Object.keys(copy).join(","));
console.log(JSON.stringify(copy));`,
    });
  }

  for (const [seed, duplicate, remove] of [
    [1, 1, 2], [2, 2, 3], [3, 1, 4], [4, 3, 5], [5, 5, 1],
    [6, 2, 7], [7, 4, 8], [8, 6, 9], [9, 3, 10], [10, 8, 11],
  ]) {
    add(cases, "collection/map-set-mutation", `edge-map-set-${seed}-${duplicate}-${remove}`, {
      lil: `Map<string,int> scores = new Map<string,int>();
scores.set("a", ${seed}).set("b", ${seed + 1});
print(scores.get("a") ?? -1);
print(scores.has("b"));
print(scores.delete("b"));
print(scores.get("b") ?? -1);
Set<int> seen = new Set<int>();
seen.add(${seed}).add(${duplicate}).add(${remove});
print(seen.has(${duplicate}));
print(seen.delete(${remove}));
print(seen.size);`,
      js: `const scores = new Map;
scores.set("a", ${seed}).set("b", ${seed + 1});
console.log(scores.get("a") ?? -1);
console.log(scores.has("b"));
console.log(scores.delete("b"));
console.log(scores.get("b") ?? -1);
const seen = new Set;
seen.add(${seed}).add(${duplicate}).add(${remove});
console.log(seen.has(${duplicate}));
console.log(seen.delete(${remove}));
console.log(seen.size);`,
    });
  }

  for (const [present, index, fallback] of [
    [true, 0, 9], [false, 0, 8], [true, 1, 7], [false, 1, 6], [true, 2, 5],
    [false, 2, 4], [true, 3, 3], [false, 3, 2], [true, 4, 1], [false, 4, 0],
  ]) {
    add(cases, "nullish/optional-indexing", `edge-nullish-optional-${present ? "present" : "missing"}-${index}-${fallback}`, {
      lil: `int calls = 0;
int nextIndex() {
  calls++;
  return ${index};
}
int fallback() {
  calls += 10;
  return ${fallback};
}
int[]? values = ${present ? "[3, 5, 7, 9, 11]" : "null"};
print(values?.[nextIndex()] ?? fallback());
print(values?.length ?? -1);
print(calls);`,
      js: `let calls = 0;
function nextIndex() {
  calls++;
  return ${index};
}
function fallback() {
  calls += 10;
  return ${fallback};
}
const values = ${present ? "[3, 5, 7, 9, 11]" : "null"};
console.log(values?.[nextIndex()] ?? fallback());
console.log(values?.length ?? -1);
console.log(calls);`,
    });
  }

  for (const [tag, text, needle, position] of [
    ["emoji-pair", "a😀b😀", "😀", 2],
    ["music", "x𝄞y𝄞z", "𝄞", 3],
    ["accent", "café-café", "é", 1],
    ["turkish", "İstanbul-istanbul", "stan", 2],
    ["greek", "αβγ-βγ", "βγ", 1],
    ["cjk", "東京-京都-東京", "東京", 2],
    ["empty", "lilscript", "", 4],
    ["miss", "compression", "zip", 3],
    ["surrogate", "🙂🙃🙂", "🙃", 1],
    ["mixed", "A😀éZ😀", "Z", 0],
  ]) {
    add(cases, "string/utf16-indexing", `edge-string-utf16-${tag}`, {
      lil: `string text = "${text}";
string needle = "${needle}";
print(text.length);
print(text.indexOf(needle, ${position}));
print(text.lastIndexOf(needle));
print(text.includes(needle));
print(text.charCodeAt(0));`,
      js: `const text = "${text}";
const needle = "${needle}";
console.log(text.length);
console.log(text.indexOf(needle, ${position}));
console.log(text.lastIndexOf(needle));
console.log(text.includes(needle));
console.log(text.charCodeAt(0));`,
    });
  }

  for (const [mode, start, increment] of [
    [0, 1, 3], [1, 2, 4], [2, 3, 5], [0, 4, 6], [1, 5, 7],
    [2, 6, 8], [0, 7, 9], [1, 8, 10], [2, 9, 11], [0, 10, 12],
  ]) {
    add(cases, "effect/exception-finally", `edge-exception-finally-${mode}-${start}-${increment}`, {
      lil: `int guarded(int mode, int start) {
  int value = start;
  try {
    value += ${increment};
    if (mode == 0) {
      throw "boom";
    }
    if (mode == 1) {
      return value;
    }
    value *= 2;
  } catch {
    value += 7;
  } finally {
    print(value);
  }
  return value;
}
print(guarded(${mode}, ${start}));`,
      js: `function guarded(mode, start) {
  let value = start;
  try {
    value = value + ${increment} | 0;
    if (mode === 0) throw "boom";
    if (mode === 1) return value;
    value = value * 2 | 0;
  } catch {
    value = value + 7 | 0;
  } finally {
    console.log(value);
  }
  return value;
}
console.log(guarded(${mode}, ${start}));`,
    });
  }

  for (const [start, stop, step] of [
    [0, 5, 1], [1, 7, 2], [2, 10, 2], [3, 12, 3], [4, 9, 1],
    [-3, 4, 2], [5, 16, 3], [6, 14, 2], [7, 11, 1], [8, 20, 4],
  ]) {
    const tag = `${start}-${stop}-${step}`.replaceAll("-", "n");
    add(cases, "effect/generator-range", `edge-generator-range-${tag}`, {
      lil: `generator int range(int start, int stop, int step) {
  for (int value = start; value < stop; value += step) {
    yield value;
  }
}
generator int values() {
  yield -1;
  yield* range(${start}, ${stop}, ${step});
}
int total = 0;
int count = 0;
for (int value of values()) {
  total += value;
  count++;
}
print(total);
print(count);`,
      js: `function* range(start, stop, step) {
  for (let value = start; value < stop; value = value + step | 0) yield value;
}
function* values() {
  yield -1;
  yield* range(${start}, ${stop}, ${step});
}
let total = 0;
let count = 0;
for (const value of values()) {
  total = total + value | 0;
  count++;
}
console.log(total);
console.log(count);`,
    });
  }

  for (const [seed, delta] of [[1, 2], [3, 5], [7, -2], [11, 4], [-5, 9]]) {
    const tag = `${seed}-${delta}`.replaceAll("-", "n");
    add(cases, "effect/async-task", `edge-async-task-${tag}`, {
      lil: `async int resolveValue(int value) {
  int resolved = await Task.resolve(value + ${delta});
  return resolved * 2;
}
resolveValue(${seed}).then((int value) => print(value));`,
      js: `async function resolveValue(value) {
  const resolved = await Promise.resolve(value + ${delta} | 0);
  return resolved * 2 | 0;
}
resolveValue(${seed}).then(value => console.log(value));`,
    });
  }

  add(cases, "host/callable-predicate", `host-callable-object`, {
    lil: `extern bool isFunctionValue(JsValue obj);
if (isFunctionValue(JS.object())) {
  print(1);
} else {
  print(0);
}`,
    js: `const value = {};
if (typeof value == "function" && typeof value.nodeType != "number" && typeof value.item != "function") {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/callable-predicate", `host-callable-fn`, {
    lil: `extern bool isFunctionValue(JsValue obj);
JsValue value = JS.method0((JsValue self) => self);
if (isFunctionValue(value)) {
  print(1);
} else {
  print(0);
}`,
    js: `const value = function(){};
if (typeof value == "function" && typeof value.nodeType != "number" && typeof value.item != "function") {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/callable-predicate", `host-callable-undefined`, {
    lil: `extern bool isFunctionValue(JsValue obj);
if (isFunctionValue(JS.undefined())) {
  print(1);
} else {
  print(0);
}`,
    js: `const value = void 0;
if (typeof value == "function" && typeof value.nodeType != "number" && typeof value.item != "function") {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/callable-detached-value", `host-callable-as-value`, {
    lil: `extern bool isFunctionValue(JsValue obj);
JsValue value = JS.method0((JsValue self) => self);
func(JsValue)->bool isFn = isFunctionValue;
if (isFn(value)) {
  print(1);
} else {
  print(0);
}`,
    js: `const isFn = a => typeof a == "function" && typeof a.nodeType != "number" && typeof a.item != "function";
if (isFn(function(){ return this; })) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/window-identity-predicate", `host-is-window-object`, {
    lil: `extern bool isWindowValue(JsValue obj);
if (isWindowValue(JS.object())) {
  print(1);
} else {
  print(0);
}`,
    js: `const value = {};
if (value != null && value === value.window) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/window-identity-predicate", `host-is-window-self`, {
    lil: `extern bool isWindowValue(JsValue obj);
JsValue value = JS.object();
JS.set(value, "window", value);
if (isWindowValue(value)) {
  print(1);
} else {
  print(0);
}`,
    js: `const value = {};
value.window = value;
if (value != null && value === value.window) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/window-identity-predicate", `host-is-window-nullish`, {
    lil: `extern bool isWindowValue(JsValue obj);
if (isWindowValue(JS.undefined())) {
  print(1);
} else {
  print(0);
}`,
    js: `const value = void 0;
if (value != null && value === value.window) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/window-document-type", `host-window-document-type`, {
    lil: `extern JsValue windowSelf();
extern JsValue windowDocument();
extern string typeOf(JsValue value);
print(typeOf(windowSelf()));
print(typeOf(windowDocument()));`,
    js: `const win = typeof window < "u" ? window : globalThis;
console.log(typeof win);
console.log(typeof win.document);`,
  });

  add(cases, "host/missing-member-fallback", `js-and-member-missing`, {
    lil: `JsValue input = JS.object();
JsValue node = JS.and(input, input["nodeName"]);
if (node is string) {
  print(node);
} else {
  print("none");
}`,
    js: `const input = {};
const node = input && input.nodeName;
if (typeof node == "string") {
  console.log(node);
} else {
  console.log("none");
}`,
  });

  add(cases, "host/define-property-configurable", `host-define-configurable`, {
    terserProperties: false,
    terserPropertyReason:
      "Object.defineProperty and Object.hasOwn share a quoted public key",
    lil: `extern void defineConfigurable(JsValue obj, string key, JsValue value);
JsValue object = JS.object();
defineConfigurable(object, "keep", JS.object());
if (JS.has(object, "keep")) {
  print(1);
} else {
  print(0);
}`,
    js: `const object = {};
Object.defineProperty(object, "keep", {value: {}, configurable: true});
if (Object.hasOwn(object, "keep")) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/iterator-assignment", `host-iterator-assign`, {
    lil: `extern void defineIterator(JsValue obj, JsValue iterator);
extern JsValue getArrayIterator();
JsValue object = JS.object();
defineIterator(object, getArrayIterator());
print(JS.typeOf(object));`,
    js: `const object = {};
object[Symbol.iterator] = Array.prototype[Symbol.iterator];
console.log(typeof object);`,
  });

  add(cases, "host/raf-nullish-type", `host-raf-or-null-type`, {
    lil: `extern JsValue requestAnimationFrameOrNull(JsValue fn);
extern string typeOf(JsValue value);
print(typeOf(requestAnimationFrameOrNull((JsValue self) => self)));`,
    js: `const raf = (typeof window < "u" ? window : globalThis).requestAnimationFrame?.(function(){});
console.log(typeof raf);`,
  });

  add(cases, "host/typeof-object", `host-typeof-plain-object`, {
    lil: `print(JS.typeOf(JS.object()));
print(JS.typeOf(JS.undefined()));
print(JS.typeOf(JS.array()));`,
    js: `console.log(typeof {});
console.log(typeof void 0);
console.log(typeof []);`,
  });

  add(cases, "boolean/typeof-predicate", `bool-typeof-predicate`, {
    lil: `bool isFn(JsValue obj) {
  return JS.typeOf(obj) == "function" && JS.typeOf(obj["nodeType"]) != "number" && JS.typeOf(obj["item"]) != "function";
}
if (isFn(JS.object())) { print(1); } else { print(0); }
JsValue fn = (JsValue a) => a;
if (isFn(fn)) { print(1); } else { print(0); }
if (isFn(JS.undefined())) { print(1); } else { print(0); }`,
    js: `const isFn = a => typeof a == "function" && typeof a.nodeType != "number" && typeof a.item != "function";
if (isFn({})) { console.log(1); } else { console.log(0); }
const fn = a => a;
if (isFn(fn)) { console.log(1); } else { console.log(0); }
if (isFn(void 0)) { console.log(1); } else { console.log(0); }`,
  });

  add(cases, "host/array-or-null", `js-array-or-null`, {
    lil: `JsValue? asArray(JsValue value) {
  if (JS.isArray(value)) {
    return value;
  }
  return null;
}
if (asArray(JS.array()) == null) { print(0); } else { print(1); }
if (asArray(JS.object()) == null) { print(0); } else { print(1); }
if (asArray(JS.undefined()) == null) { print(0); } else { print(1); }`,
    js: `const asArray = value => Array.isArray(value) ? value : null;
if (asArray([]) == null) { console.log(0); } else { console.log(1); }
if (asArray({}) == null) { console.log(0); } else { console.log(1); }
if (asArray(void 0) == null) { console.log(0); } else { console.log(1); }`,
  });

  add(cases, "host/amd-define-guard", `amd-define-guard`, {
    terserProperties: false,
    terserPropertyReason:
      "amd is a public property supplied by an open-world module loader",
    lil: `extern JsValue windowSelf();
JsValue root = windowSelf();
JsValue define = root["define"];
if (JS.typeOf(define) == "function" && define["amd"].truthy()) {
  print(1);
} else {
  print(0);
}`,
    js: `const root = typeof window < "u" ? window : globalThis;
const define = root.define;
if (typeof define == "function" && define.amd) {
  console.log(1);
} else {
  console.log(0);
}`,
  });

  add(cases, "host/window-repeated-read", `host-window-repeat`, {
    lil: `extern JsValue windowSelf();
print(JS.typeOf(windowSelf()["document"]));
print(JS.typeOf(windowSelf()["location"]));
print(JS.typeOf(windowSelf()["console"]));`,
    js: `const w = typeof window < "u" ? window : globalThis;
console.log(typeof w.document);
console.log(typeof w.location);
console.log(typeof w.console);`,
  });

  add(cases, "integer/division-by-zero", "semantic-integer-division-by-zero", {
    lil: `int divide(int value, int divisor) {
  return value / divisor;
}
print(divide(17, 0));
print(divide(-17, 0));`,
    js: `function divide(value, divisor) {
  return value / divisor | 0;
}
console.log(divide(17, 0));
console.log(divide(-17, 0));`,
  });

  add(cases, "integer/remainder-by-zero", "semantic-integer-remainder-by-zero", {
    lil: `int remainder(int value, int divisor) {
  return value % divisor;
}
print(remainder(17, 0));
print(remainder(-17, 0));`,
    js: `function remainder(value, divisor) {
  return value % divisor | 0;
}
console.log(remainder(17, 0));
console.log(remainder(-17, 0));`,
  });

  add(cases, "integer/shift-count-masking", "semantic-integer-shift-count-masking", {
    lil: `print(1 << 33);`,
    js: `console.log(1 << 33);`,
  });

  add(cases, "integer/update-expression-values", "semantic-integer-update-values", {
    lil: `int value = 4;
print(value++);
print(++value);
print(value--);
print(--value);
print(value);`,
    js: `let value = 4;
console.log(value++);
console.log(++value);
console.log(value--);
console.log(--value);
console.log(value);`,
  });

  add(cases, "integer/radix-formatting", "semantic-integer-radix-formatting", {
    lil: `int negative = -1;
print(negative.toString(16));
print(negative.toUnsignedString(16));
print(35.toString(36));`,
    js: `const negative = -1;
console.log(negative.toString(16));
console.log((negative >>> 0).toString(16));
console.log((35).toString(36));`,
  });

  add(cases, "nullish/lazy-assignment", "semantic-nullish-lazy-assignment", {
    lil: `int calls = 0;
int fallback() {
  calls++;
  return 7;
}
int? value = null;
int assigned = value ??= fallback();
value ??= fallback();
print(assigned);
print(value ?? 0);
print(calls);`,
    js: `let calls = 0;
function fallback() {
  calls++;
  return 7;
}
let value = null;
const assigned = value ??= fallback();
value ??= fallback();
console.log(assigned);
console.log(value ?? 0);
console.log(calls);`,
  });

  add(cases, "number/math-intrinsics", "semantic-number-math-intrinsics", {
    lil: `print(9.0.sqrt());
print((-4.0).abs());
print(1.5.round());`,
    js: `console.log(Math.sqrt(9));
console.log(Math.abs(-4));
console.log(Math.round(1.5));`,
  });

  add(cases, "string/repeat", "semantic-string-repeat", {
    lil: `print("ab".repeat(3));
print("x".repeat(0).length);`,
    js: `console.log("ab".repeat(3));
console.log("x".repeat(0).length);`,
  });

  add(cases, "string/code-point-length", "semantic-string-code-point-length", {
    lil: `print("A😀é".length);
print("A😀é".codePointLength());`,
    js: `console.log("A😀é".length);
console.log([..."A😀é"].length);`,
  });

  add(cases, "string/out-of-range-code-unit", "semantic-string-out-of-range-code-unit", {
    lil: `string value = "abc";
print(value.charCodeAt(99));
print(value.charAt(99));`,
    js: `const value = "abc";
console.log(value.charCodeAt(99) || 0);
console.log(value.charAt(99));`,
  });

  add(cases, "collection/array-splice", "semantic-array-splice", {
    lil: `int[] values = [1, 2, 3, 4, 5];
int[] removed = values.splice(-3, 2);
print(removed.join("-"));
print(values.join("-"));`,
    js: `const values = [1, 2, 3, 4, 5];
const removed = values.splice(-3, 2);
console.log(removed.join("-"));
console.log(values.join("-"));`,
  });

  add(cases, "collection/array-nan-membership", "semantic-array-nan-membership", {
    lil: `float nan = 0.0 / 0.0;
float[] values = [nan];
print(values.includes(nan));
print(values.indexOf(nan));
print(values.includes(nan, -1));
print(values.indexOf(nan));`,
    js: `const nan = 0 / 0;
const values = [nan];
console.log(values.includes(nan));
console.log(values.indexOf(nan));
console.log(values.includes(nan, -1));
console.log(values.indexOf(nan));`,
  });

  add(cases, "number/to-int-boundaries", "semantic-number-to-int-boundaries", {
    lil: `print(3.9.toInt());
print((-3.9).toInt());
print(4294967297.0.toInt());
print(2147483648.0.toInt());`,
    js: `console.log(3.9 | 0);
console.log(-3.9 | 0);
console.log(4294967297 | 0);
console.log(2147483648 | 0);`,
  });

  add(cases, "integer/minimum-divide-negative-one", "semantic-integer-min-div-neg-one", {
    lil: `int minimum = -2147483647 - 1;
print(minimum / -1);`,
    js: `const minimum = -2147483647 - 1;
console.log(minimum / -1 | 0);`,
  });

  add(cases, "collection/array-copy-within", "semantic-array-copy-within", {
    lil: `int[] values = [1, 2, 3, 4, 5];
values.copyWithin(1, 3);
print(values.join("-"));`,
    js: `const values = [1, 2, 3, 4, 5];
values.copyWithin(1, 3);
console.log(values.join("-"));`,
  });

  add(cases, "collection/array-fill", "semantic-array-fill", {
    lil: `int[] values = [1, 2, 3, 4];
values.fill(9);
print(values.join("-"));`,
    js: `const values = [1, 2, 3, 4];
values.fill(9);
console.log(values.join("-"));`,
  });

  add(cases, "string/split-empty-fields", "semantic-string-split-empty-fields", {
    lil: `string[] parts = "a,,b,".split(",");
print(parts.length);
print(parts.join("|"));`,
    js: `const parts = "a,,b,".split(",");
console.log(parts.length);
console.log(parts.join("|"));`,
  });

  add(cases, "collection/array-some-short-circuit", "semantic-array-some-short-circuit", {
    lil: `int calls = 0;
int[] values = [1, 2, 3, 4];
bool found = values.some((int value) => {
  calls++;
  return value == 2;
});
print(found);
print(calls);`,
    js: `let calls = 0;
const values = [1, 2, 3, 4];
const found = values.some(value => {
  calls++;
  return value === 2;
});
console.log(found);
console.log(calls);`,
  });

  add(cases, "function/default-array-freshness", "semantic-default-array-freshness", {
    lil: `int append(int[] values = []) {
  values.push(1);
  return values.length;
}
print(append());
print(append());`,
    js: `function append(values = []) {
  values.push(1);
  return values.length;
}
console.log(append());
console.log(append());`,
  });

  add(cases, "union/string-int-flow", "semantic-union-string-int-flow", {
    lil: `string|int choose(bool text) {
  if (text) { return "hello"; }
  return 42;
}
string|int first = choose(true);
string|int second = choose(false);
print(first);
print(second);`,
    js: `function choose(text) {
  if (text) return "hello";
  return 42;
}
const first = choose(true);
const second = choose(false);
console.log(first);
console.log(second);`,
  });

  add(cases, "function/array-dispatch", "semantic-function-array-dispatch", {
    lil: `int doubleValue(int value) { return value * 2; }
int increment(int value) { return value + 1; }
(func(int)->int)[] transforms = [doubleValue, increment];
int value = 3;
for (int index = 0; index < transforms.length; index++) {
  value = transforms[index](value);
}
print(value);`,
    js: `function doubleValue(value) { return value * 2 | 0; }
function increment(value) { return value + 1 | 0; }
const transforms = [doubleValue, increment];
let value = 3;
for (let index = 0; index < transforms.length; index++) {
  value = transforms[index](value);
}
console.log(value);`,
  });

  add(cases, "string/template-interpolation", "semantic-template-interpolation", {
    lil: `string language = "Lil" + "Script";
int version = 1;
bool stable = true;
print(\`\${language}:\${version}:\${stable}\`);`,
    js: `const language = "Lil" + "Script";
const version = 1;
const stable = true;
console.log(\`\${language}:\${version}:\${stable}\`);`,
  });

  add(cases, "generic/equality-object-identity", "frontier-generic-equality", {
    lil: `bool same<T>(T left, T right) {
  return left == right;
}
class Box {
  int value;
  init(int value) { this.value = value; }
}
Box shared = new Box(1);
print(same(7, 7));
print(same("lil", "script"));
print(same(shared, shared));
print(same(shared, new Box(1)));`,
    js: `function same(left, right) {
  return left === right;
}
class Box {
  constructor(value) { this.value = value; }
}
const shared = new Box(1);
console.log(same(7, 7));
console.log(same("lil", "script"));
console.log(same(shared, shared));
console.log(same(shared, new Box(1)));`,
  });

  add(cases, "control/inline-for-unrolling", "frontier-inline-for", {
    lil: `int total = 0;
inline for (int value of [1, 2, 3, 4]) {
  total += value;
}
print(total);
string joined = "";
inline for (string part of ["ab", "cd"]) {
  joined = joined + part;
}
print(joined);`,
    js: `let total = 0;
for (const value of [1, 2, 3, 4]) {
  total = total + value | 0;
}
console.log(total);
let joined = "";
for (const part of ["ab", "cd"]) {
  joined += part;
}
console.log(joined);`,
  });

  add(cases, "aggregate/default-constructor-freshness", "frontier-default-constructor", {
    lil: `class Counter {
  int value;
  init() { this.value = 0; }
  int increment() {
    this.value += 1;
    return this.value;
  }
}
int bump(Counter counter = new Counter()) {
  return counter.increment();
}
print(bump());
print(bump());`,
    js: `class Counter {
  constructor() { this.value = 0; }
  increment() {
    this.value += 1;
    return this.value;
  }
}
function bump(counter = new Counter()) {
  return counter.increment();
}
console.log(bump());
console.log(bump());`,
  });

  add(cases, "control/enum-lazy-match", "frontier-enum-lazy-match", {
    lil: `enum Status { Draft, Active, Sold }
int calls = 0;
string mark(string value) {
  calls++;
  return value;
}
string label(Status value) {
  return match (value) {
    Status.Draft => mark("draft"),
    Status.Active => mark("active"),
    Status.Sold => mark("sold")
  };
}
print(label(Status.Active));
print(calls);`,
    js: `let calls = 0;
function mark(value) {
  calls++;
  return value;
}
function label(value) {
  return value === 0 ? mark("draft") : value === 1 ? mark("active") : mark("sold");
}
console.log(label(1));
console.log(calls);`,
  });

  add(cases, "collection/array-spread-order", "frontier-array-spread-order", {
    lil: `int calls = 0;
int[] source() {
  calls++;
  return [1, 2];
}
int[] values = [0, ...source(), 3];
print(values.join("-"));
print(calls);`,
    js: `let calls = 0;
function source() {
  calls++;
  return [1, 2];
}
const values = [0, ...source(), 3];
console.log(values.join("-"));
console.log(calls);`,
  });

  add(cases, "aggregate/inheritance-super", "frontier-inheritance-super", {
    lil: `class Base {
  int value;
  init(int value) { this.value = value; }
  int get() { return this.value; }
}
class Child extends Base {
  int bonus;
  init(int value, int bonus) {
    super(value);
    this.bonus = bonus;
  }
  int total() { return this.get() + this.bonus; }
}
Child child = new Child(4, 3);
print(child.total());`,
    js: `class Base {
  constructor(value) { this.value = value; }
  get() { return this.value; }
}
class Child extends Base {
  constructor(value, bonus) {
    super(value);
    this.bonus = bonus;
  }
  total() { return this.get() + this.bonus | 0; }
}
const child = new Child(4, 3);
console.log(child.total());`,
  });

  return cases;
}
