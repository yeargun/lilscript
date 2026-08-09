import assert from "node:assert/strict";

const upstream = await import("gl-matrix/esm/index.js");
const lilscript = await import("./build/gl-matrix-pm/api.js");
const modules = [
  "glMatrix",
  "mat2",
  "mat2d",
  "mat3",
  "mat4",
  "quat",
  "quat2",
  "vec2",
  "vec3",
  "vec4",
];

assert.deepEqual(Object.keys(lilscript).sort(), Object.keys(upstream).sort());
for (const name of modules) {
  assert.deepEqual(
    Object.keys(lilscript[name]).sort(),
    Object.keys(upstream[name]).sort(),
    `${name} exports`,
  );
  assert.equal(Object.getPrototypeOf(lilscript[name]), null, `${name} namespace prototype`);
  assert.equal(Object.isExtensible(lilscript[name]), false, `${name} namespace extensibility`);
  assert.equal(
    lilscript[name][Symbol.toStringTag],
    upstream[name][Symbol.toStringTag],
    `${name} namespace tag`,
  );
  for (const key of Object.keys(upstream[name])) {
    const expected = upstream[name][key];
    const actual = lilscript[name][key];
    assert.equal(typeof actual, typeof expected, `${name}.${key} type`);
    if (typeof expected === "function") {
      assert.equal(actual.length, expected.length, `${name}.${key} arity`);
      const expectedConstructor = (() => {
        try {
          Reflect.construct(String, [], expected);
          return true;
        } catch {
          return false;
        }
      })();
      const actualConstructor = (() => {
        try {
          Reflect.construct(String, [], actual);
          return true;
        } catch {
          return false;
        }
      })();
      assert.equal(actualConstructor, expectedConstructor, `${name}.${key} constructibility`);
    }
  }
}

assert.equal(lilscript.glMatrix.ARRAY_TYPE, Float32Array);
assert.equal(lilscript.glMatrix.RANDOM, upstream.glMatrix.RANDOM);
lilscript.glMatrix.setMatrixArrayType(Array);
assert.equal(lilscript.glMatrix.ARRAY_TYPE, Array);
assert.ok(Array.isArray(lilscript.vec4.create()));
lilscript.glMatrix.setMatrixArrayType(Float32Array);
assert.ok(lilscript.vec4.create() instanceof Float32Array);
class CustomArray extends Array {}
lilscript.glMatrix.setMatrixArrayType(CustomArray);
assert.ok(lilscript.vec4.create() instanceof CustomArray);
lilscript.glMatrix.setMatrixArrayType(Float32Array);

for (const module of [upstream, lilscript]) {
  for (const scale of [undefined, 0, -0, Number.NaN, 2, -3]) {
    const expectedLength = Math.abs(scale || 1);
    for (const vector of [module.vec2, module.vec3, module.vec4]) {
      const dimensions = vector === module.vec2 ? 2 : vector === module.vec3 ? 3 : 4;
      const out = new Float32Array(dimensions);
      vector.random(out, scale);
      const length = Math.sqrt([...out].reduce((sum, value) => sum + value * value, 0));
      assert.ok(
        Math.abs(length - expectedLength) <= 1e-5,
        `${vector === module.vec2 ? "vec2" : vector === module.vec3 ? "vec3" : "vec4"}.random(${String(scale)})`,
      );
    }
  }
}

const exportCount = modules.reduce(
  (total, name) => total + Object.keys(upstream[name]).length,
  0,
);
console.log(`gl-matrix-upstream:${modules.length}:${exportCount}`);
