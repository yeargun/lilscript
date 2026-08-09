const close = (actual, expected, tolerance = 0.00001) =>
  Math.abs(actual - expected) <= tolerance;

const valuesClose = (actual, expected) =>
  actual.length === expected.length &&
  expected.every((value, index) => close(actual[index], value));

export function runRootContract(root) {
  const expected =
    "glMatrix,mat2,mat2d,mat3,mat4,quat,quat2,vec2,vec3,vec4";
  if (Object.keys(root).sort().join(",") !== expected) {
    throw new Error("root export contract failed");
  }
  console.log("gl-matrix-root:1");
}

export function runCommonContract(glMatrix, vec2) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`common contract failed at ${passed + 1}`);
    passed += 1;
  };

  check(
    Object.keys(glMatrix).sort().join(",") ===
      "ARRAY_TYPE,EPSILON,RANDOM,equals,setMatrixArrayType,toRadian",
  );
  check(glMatrix.EPSILON === 0.000001);
  check(glMatrix.ARRAY_TYPE === Float32Array);
  check(glMatrix.RANDOM === Math.random);
  check(close(glMatrix.toRadian(180), Math.PI));
  check(glMatrix.equals(1, 1.0000005) && !glMatrix.equals(1, 1.000002));
  check(vec2.create() instanceof Float32Array);
  glMatrix.setMatrixArrayType(Array);
  check(glMatrix.ARRAY_TYPE === Array && Array.isArray(vec2.create()));
  glMatrix.setMatrixArrayType(Float32Array);
  check(glMatrix.ARRAY_TYPE === Float32Array && vec2.create() instanceof Float32Array);
  class CustomArray extends Array {}
  glMatrix.setMatrixArrayType(CustomArray);
  check(glMatrix.ARRAY_TYPE === CustomArray && vec2.create() instanceof CustomArray);
  glMatrix.setMatrixArrayType(Float32Array);

  console.log(`gl-matrix-common:${passed}`);
}

export function runVec2Contract(vec2) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`vec2 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const a = vec2.fromValues(3, 4);
  const b = vec2.fromValues(1, 2);
  const out = vec2.create();

  check(a instanceof Float32Array && valuesClose(a, [3, 4]));
  check(valuesClose(vec2.clone(a), [3, 4]));
  check(vec2.copy(out, b) === out && valuesClose(out, [1, 2]));
  check(vec2.set(out, 5, 6) === out && valuesClose(out, [5, 6]));
  check(valuesClose(vec2.add(out, a, b), [4, 6]));
  check(valuesClose(vec2.subtract(out, a, b), [2, 2]));
  check(valuesClose(vec2.multiply(out, a, b), [3, 8]));
  check(valuesClose(vec2.divide(out, a, b), [3, 2]));
  check(valuesClose(vec2.ceil(out, vec2.fromValues(1.2, -1.8)), [2, -1]));
  check(valuesClose(vec2.floor(out, vec2.fromValues(1.8, -1.2)), [1, -2]));
  check(valuesClose(vec2.min(out, a, b), [1, 2]));
  check(valuesClose(vec2.max(out, a, b), [3, 4]));
  check(valuesClose(vec2.round(out, vec2.fromValues(1.5, -1.5)), [2, -1]));
  check(valuesClose(vec2.scale(out, a, 2), [6, 8]));
  check(valuesClose(vec2.scaleAndAdd(out, a, b, 2), [5, 8]));
  check(close(vec2.distance(a, b), Math.sqrt(8)));
  check(vec2.squaredDistance(a, b) === 8);
  check(vec2.length(a) === 5 && vec2.squaredLength(a) === 25);
  check(valuesClose(vec2.negate(out, b), [-1, -2]));
  check(valuesClose(vec2.inverse(out, b), [1, 0.5]));
  check(valuesClose(vec2.normalize(out, a), [0.6, 0.8]));
  check(vec2.dot(a, b) === 11);

  const cross = new Float32Array(3);
  check(vec2.cross(cross, a, b) === cross && valuesClose(cross, [0, 0, 2]));
  check(valuesClose(vec2.lerp(out, a, b, 0.25), [2.5, 3.5]));
  check(valuesClose(vec2.transformMat2(out, b, new Float32Array([1, 2, 3, 4])), [7, 10]));
  check(
    valuesClose(
      vec2.transformMat2d(out, b, new Float32Array([1, 2, 3, 4, 5, 6])),
      [12, 16],
    ),
  );
  check(
    valuesClose(
      vec2.transformMat3(out, b, new Float32Array([1, 2, 0, 3, 4, 0, 5, 6, 1])),
      [12, 16],
    ),
  );

  const mat4 = new Float32Array(16);
  mat4[0] = 1;
  mat4[1] = 2;
  mat4[4] = 3;
  mat4[5] = 4;
  mat4[12] = 5;
  mat4[13] = 6;
  check(valuesClose(vec2.transformMat4(out, b, mat4), [12, 16]));
  check(
    valuesClose(
      vec2.rotate(out, vec2.fromValues(1, 0), vec2.fromValues(0, 0), Math.PI / 2),
      [0, 1],
    ),
  );
  check(close(vec2.angle(vec2.fromValues(1, 0), vec2.fromValues(0, 1)), Math.PI / 2));
  check(valuesClose(vec2.zero(out), [0, 0]));
  check(vec2.str(vec2.fromValues(1, 2)) === "vec2(1, 2)");
  check(vec2.exactEquals(a, vec2.fromValues(3, 4)));
  check(vec2.equals(a, vec2.fromValues(3.000001, 4.000001)));
  check(
    vec2.len === vec2.length &&
      vec2.sub === vec2.subtract &&
      vec2.mul === vec2.multiply &&
      vec2.div === vec2.divide &&
      vec2.dist === vec2.distance &&
      vec2.sqrDist === vec2.squaredDistance &&
      vec2.sqrLen === vec2.squaredLength,
  );

  const packed = new Float32Array([1, 2, 3, 4]);
  check(
    vec2.forEach(
      packed,
      2,
      0,
      0,
      (target, value, amount) => vec2.scale(target, value, amount),
      2,
    ) === packed && valuesClose(packed, [2, 4, 6, 8]),
  );
  const arrayOut = [0, 0];
  check(
    vec2.add(arrayOut, [1, 2], [3, 4]) === arrayOut &&
      valuesClose(arrayOut, [4, 6]),
  );

  console.log(`gl-matrix-vec2:${passed}`);
}

export function runMat2Contract(mat2) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`mat2 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const a = mat2.fromValues(4, 7, 2, 6);
  const b = mat2.fromValues(1, 2, 3, 4);
  const out = mat2.create();

  check(valuesClose(out, [1, 0, 0, 1]));
  check(valuesClose(mat2.clone(a), [4, 7, 2, 6]));
  check(mat2.copy(out, b) === out && valuesClose(out, [1, 2, 3, 4]));
  check(mat2.identity(out) === out && valuesClose(out, [1, 0, 0, 1]));
  check(mat2.set(out, 5, 6, 7, 8) === out && valuesClose(out, [5, 6, 7, 8]));
  check(valuesClose(mat2.transpose(out, b), [1, 3, 2, 4]));

  const selfTranspose = mat2.clone(b);
  check(
    mat2.transpose(selfTranspose, selfTranspose) === selfTranspose &&
      valuesClose(selfTranspose, [1, 3, 2, 4]),
  );
  check(valuesClose(mat2.invert(out, a), [0.6, -0.7, -0.2, 0.4]));
  check(mat2.invert(out, mat2.fromValues(1, 2, 2, 4)) === null);
  check(valuesClose(mat2.adjoint(out, a), [6, -7, -2, 4]));
  check(mat2.determinant(a) === 10);
  check(valuesClose(mat2.multiply(out, a, b), [8, 19, 20, 45]));
  check(
    valuesClose(mat2.rotate(out, mat2.create(), Math.PI / 2), [0, 1, -1, 0]),
  );
  check(valuesClose(mat2.scale(out, b, [2, 3]), [2, 4, 9, 12]));
  check(valuesClose(mat2.fromRotation(out, Math.PI / 2), [0, 1, -1, 0]));
  check(valuesClose(mat2.fromScaling(out, [2, 3]), [2, 0, 0, 3]));
  check(mat2.str(b) === "mat2(1, 2, 3, 4)");
  check(close(mat2.frob(b), Math.sqrt(30)));

  const lower = mat2.create();
  const diagonal = mat2.create();
  const upper = mat2.create();
  const factors = mat2.LDU(lower, diagonal, upper, a);
  check(
    factors[0] === lower &&
      factors[1] === diagonal &&
      factors[2] === upper &&
      close(lower[2], 0.5) &&
      valuesClose(upper, [4, 7, 0, 2.5]),
  );
  check(valuesClose(mat2.add(out, a, b), [5, 9, 5, 10]));
  check(valuesClose(mat2.subtract(out, a, b), [3, 5, -1, 2]));
  check(mat2.exactEquals(a, mat2.fromValues(4, 7, 2, 6)));
  check(mat2.equals(a, mat2.fromValues(4.000001, 7.000001, 2.000001, 6.000001)));
  check(valuesClose(mat2.multiplyScalar(out, b, 2), [2, 4, 6, 8]));
  check(valuesClose(mat2.multiplyScalarAndAdd(out, a, b, 2), [6, 11, 8, 14]));
  check(mat2.mul === mat2.multiply && mat2.sub === mat2.subtract);

  console.log(`gl-matrix-mat2:${passed}`);
}

export function runMat2dContract(mat2d) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`mat2d contract failed at ${passed + 1}`);
    passed += 1;
  };

  const a = mat2d.fromValues(1, 2, 3, 4, 5, 6);
  const b = mat2d.fromValues(2, 0, 0, 3, 7, 8);
  const out = mat2d.create();

  check(valuesClose(out, [1, 0, 0, 1, 0, 0]));
  check(valuesClose(mat2d.clone(a), [1, 2, 3, 4, 5, 6]));
  check(mat2d.copy(out, b) === out && valuesClose(out, [2, 0, 0, 3, 7, 8]));
  check(mat2d.identity(out) === out && valuesClose(out, [1, 0, 0, 1, 0, 0]));
  check(
    mat2d.set(out, 6, 5, 4, 3, 2, 1) === out &&
      valuesClose(out, [6, 5, 4, 3, 2, 1]),
  );
  check(valuesClose(mat2d.invert(out, a), [-2, 1, 1.5, -0.5, 1, -2]));
  check(mat2d.invert(out, mat2d.fromValues(1, 2, 2, 4, 0, 0)) === null);
  check(mat2d.determinant(a) === -2);
  check(valuesClose(mat2d.multiply(out, a, b), [2, 4, 9, 12, 36, 52]));
  check(
    valuesClose(
      mat2d.rotate(out, mat2d.create(), Math.PI / 2),
      [0, 1, -1, 0, 0, 0],
    ),
  );
  check(valuesClose(mat2d.scale(out, a, [2, 3]), [2, 4, 9, 12, 5, 6]));
  check(valuesClose(mat2d.translate(out, a, [2, 3]), [1, 2, 3, 4, 16, 22]));
  check(
    valuesClose(mat2d.fromRotation(out, Math.PI / 2), [0, 1, -1, 0, 0, 0]),
  );
  check(valuesClose(mat2d.fromScaling(out, [2, 3]), [2, 0, 0, 3, 0, 0]));
  check(valuesClose(mat2d.fromTranslation(out, [2, 3]), [1, 0, 0, 1, 2, 3]));
  check(mat2d.str(a) === "mat2d(1, 2, 3, 4, 5, 6)");
  check(close(mat2d.frob(a), Math.sqrt(92)));
  check(valuesClose(mat2d.add(out, a, b), [3, 2, 3, 7, 12, 14]));
  check(valuesClose(mat2d.subtract(out, a, b), [-1, 2, 3, 1, -2, -2]));
  check(valuesClose(mat2d.multiplyScalar(out, a, 2), [2, 4, 6, 8, 10, 12]));
  check(
    valuesClose(
      mat2d.multiplyScalarAndAdd(out, a, b, 2),
      [5, 2, 3, 10, 19, 22],
    ),
  );
  check(mat2d.exactEquals(a, mat2d.fromValues(1, 2, 3, 4, 5, 6)));
  check(
    mat2d.equals(a, mat2d.fromValues(1.000001, 2.000001, 3.000001, 4.000001, 5.000001, 6.000001)),
  );
  check(mat2d.mul === mat2d.multiply && mat2d.sub === mat2d.subtract);

  console.log(`gl-matrix-mat2d:${passed}`);
}

const withStubbedRandom = (sequence, run) => {
  const original = Math.random;
  let index = 0;
  Math.random = () => sequence[index++ % sequence.length];
  try {
    return run();
  } finally {
    Math.random = original;
  }
};

export function runVec3Contract(vec3) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`vec3 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const a = vec3.fromValues(1, 2, 3);
  const b = vec3.fromValues(4, 5, 6);
  const out = vec3.create();

  check(a instanceof Float32Array && valuesClose(a, [1, 2, 3]));
  check(valuesClose(out, [0, 0, 0]));
  check(valuesClose(vec3.clone(a), [1, 2, 3]));
  check(vec3.copy(out, b) === out && valuesClose(out, [4, 5, 6]));
  check(vec3.set(out, 7, 8, 9) === out && valuesClose(out, [7, 8, 9]));
  check(valuesClose(vec3.add(out, a, b), [5, 7, 9]));
  check(valuesClose(vec3.subtract(out, a, b), [-3, -3, -3]));
  check(valuesClose(vec3.multiply(out, a, b), [4, 10, 18]));
  check(valuesClose(vec3.divide(out, b, a), [4, 2.5, 2]));
  check(valuesClose(vec3.ceil(out, vec3.fromValues(1.2, -1.8, 2.1)), [2, -1, 3]));
  check(valuesClose(vec3.floor(out, vec3.fromValues(1.8, -1.2, 2.9)), [1, -2, 2]));
  check(valuesClose(vec3.min(out, a, b), [1, 2, 3]));
  check(valuesClose(vec3.max(out, a, b), [4, 5, 6]));
  check(valuesClose(vec3.round(out, vec3.fromValues(1.5, -1.5, 2.5)), [2, -1, 3]));
  check(valuesClose(vec3.scale(out, a, 2), [2, 4, 6]));
  check(valuesClose(vec3.scaleAndAdd(out, a, b, 2), [9, 12, 15]));
  check(close(vec3.distance(a, b), Math.sqrt(27)));
  check(vec3.squaredDistance(a, b) === 27);
  check(close(vec3.length(a), Math.sqrt(14)) && vec3.squaredLength(a) === 14);
  check(valuesClose(vec3.negate(out, a), [-1, -2, -3]));
  check(valuesClose(vec3.inverse(out, a), [1, 0.5, 1 / 3]));
  check(
    valuesClose(vec3.normalize(out, a), [
      0.26726124,
      0.53452247,
      0.80178374,
    ]),
  );
  check(vec3.dot(a, b) === 32);
  check(valuesClose(vec3.cross(out, a, b), [-3, 6, -3]));
  check(valuesClose(vec3.lerp(out, a, b, 0.5), [2.5, 3.5, 4.5]));
  check(
    valuesClose(
      vec3.hermite(
        out,
        a,
        b,
        vec3.fromValues(0, 1, 0),
        vec3.fromValues(1, 0, 1),
        0.5,
      ),
      [1.5, 1.5, 2.75],
    ),
  );
  check(
    valuesClose(
      vec3.bezier(
        out,
        a,
        b,
        vec3.fromValues(0, 1, 0),
        vec3.fromValues(1, 0, 1),
        0.5,
      ),
      [1.75, 2.5, 2.75],
    ),
  );
  withStubbedRandom([0.25, 0.25], () => {
    const random = vec3.random(out, 2);
    check(random === out && close(vec3.length(out), 2));
  });
  check(
    valuesClose(
      vec3.transformMat4(
        out,
        a,
        new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1]),
      ),
      [11, 22, 33],
    ),
  );
  check(
    valuesClose(
      vec3.transformMat3(out, a, new Float32Array([1, 0, 0, 0, 1, 0, 0, 0, 1])),
      [1, 2, 3],
    ),
  );
  check(
    valuesClose(
      vec3.transformQuat(
        out,
        vec3.fromValues(1, 0, 0),
        new Float32Array([0, 0, Math.sin(Math.PI / 4), Math.cos(Math.PI / 4)]),
      ),
      [0, 1, 0],
    ),
  );
  check(
    valuesClose(
      vec3.rotateX(out, vec3.fromValues(0, 1, 0), vec3.fromValues(0, 0, 0), Math.PI / 2),
      [0, 0, 1],
    ),
  );
  check(
    valuesClose(
      vec3.rotateY(out, vec3.fromValues(1, 0, 0), vec3.fromValues(0, 0, 0), Math.PI / 2),
      [0, 0, -1],
    ),
  );
  check(
    valuesClose(
      vec3.rotateZ(out, vec3.fromValues(1, 0, 0), vec3.fromValues(0, 0, 0), Math.PI / 2),
      [0, 1, 0],
    ),
  );
  check(close(vec3.angle(vec3.fromValues(1, 0, 0), vec3.fromValues(0, 1, 0)), Math.PI / 2));
  check(valuesClose(vec3.zero(out), [0, 0, 0]));
  check(vec3.str(vec3.fromValues(1, 2, 3)) === "vec3(1, 2, 3)");
  check(vec3.exactEquals(a, vec3.fromValues(1, 2, 3)));
  check(vec3.equals(a, vec3.fromValues(1.000001, 2.000001, 3.000001)));
  check(
    vec3.len === vec3.length &&
      vec3.sub === vec3.subtract &&
      vec3.mul === vec3.multiply &&
      vec3.div === vec3.divide &&
      vec3.dist === vec3.distance &&
      vec3.sqrDist === vec3.squaredDistance &&
      vec3.sqrLen === vec3.squaredLength,
  );
  const packed = new Float32Array([1, 2, 3, 4, 5, 6]);
  check(
    vec3.forEach(
      packed,
      3,
      0,
      0,
      (target, value, amount) => vec3.scale(target, value, amount),
      2,
    ) === packed && valuesClose(packed, [2, 4, 6, 8, 10, 12]),
  );
  const arrayOut = [0, 0, 0];
  check(
    vec3.add(arrayOut, [1, 2, 3], [4, 5, 6]) === arrayOut &&
      valuesClose(arrayOut, [5, 7, 9]),
  );

  console.log(`gl-matrix-vec3:${passed}`);
}

export function runVec4Contract(vec4) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`vec4 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const a = vec4.fromValues(1, 2, 3, 4);
  const b = vec4.fromValues(5, 6, 7, 8);
  const out = vec4.create();

  check(a instanceof Float32Array && valuesClose(a, [1, 2, 3, 4]));
  check(valuesClose(out, [0, 0, 0, 0]));
  check(valuesClose(vec4.clone(a), [1, 2, 3, 4]));
  check(vec4.copy(out, b) === out && valuesClose(out, [5, 6, 7, 8]));
  check(vec4.set(out, 9, 8, 7, 6) === out && valuesClose(out, [9, 8, 7, 6]));
  check(valuesClose(vec4.add(out, a, b), [6, 8, 10, 12]));
  check(valuesClose(vec4.subtract(out, b, a), [4, 4, 4, 4]));
  check(valuesClose(vec4.multiply(out, a, b), [5, 12, 21, 32]));
  check(valuesClose(vec4.divide(out, b, a), [5, 3, 7 / 3, 2]));
  check(valuesClose(vec4.ceil(out, vec4.fromValues(1.2, -1.8, 2.1, -0.1)), [2, -1, 3, 0]));
  check(valuesClose(vec4.floor(out, vec4.fromValues(1.8, -1.2, 2.9, -0.1)), [1, -2, 2, -1]));
  check(valuesClose(vec4.min(out, a, b), [1, 2, 3, 4]));
  check(valuesClose(vec4.max(out, a, b), [5, 6, 7, 8]));
  check(valuesClose(vec4.round(out, vec4.fromValues(1.5, -1.5, 2.5, -2.5)), [2, -1, 3, -2]));
  check(valuesClose(vec4.scale(out, a, 2), [2, 4, 6, 8]));
  check(valuesClose(vec4.scaleAndAdd(out, a, b, 2), [11, 14, 17, 20]));
  check(close(vec4.distance(a, b), Math.sqrt(64)));
  check(vec4.squaredDistance(a, b) === 64);
  check(close(vec4.length(a), Math.sqrt(30)) && vec4.squaredLength(a) === 30);
  check(valuesClose(vec4.negate(out, a), [-1, -2, -3, -4]));
  check(valuesClose(vec4.inverse(out, a), [1, 0.5, 1 / 3, 0.25]));
  check(close(vec4.length(vec4.normalize(out, a)), 1));
  check(vec4.dot(a, b) === 70);
  check(
    valuesClose(
      vec4.cross(out, [1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]),
      [0, 0, 0, -1],
    ),
  );
  check(valuesClose(vec4.lerp(out, a, b, 0.25), [2, 3, 4, 5]));
  withStubbedRandom([0.1, 0.2, 0.3, 0.4], () => {
    const random = vec4.random(out, 2);
    check(random === out && close(vec4.length(out), 2));
  });
  check(
    valuesClose(
      vec4.transformMat4(
        out,
        a,
        new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1]),
      ),
      [41, 82, 123, 4],
    ),
  );
  check(
    valuesClose(
      vec4.transformQuat(
        out,
        vec4.fromValues(1, 0, 0, 1),
        new Float32Array([0, 0, Math.sin(Math.PI / 4), Math.cos(Math.PI / 4)]),
      ),
      [0, 1, 0, 1],
    ),
  );
  check(valuesClose(vec4.zero(out), [0, 0, 0, 0]));
  check(vec4.str(vec4.fromValues(1, 2, 3, 4)) === "vec4(1, 2, 3, 4)");
  check(vec4.exactEquals(a, vec4.fromValues(1, 2, 3, 4)));
  check(vec4.equals(a, vec4.fromValues(1.000001, 2.000001, 3.000001, 4.000001)));
  check(
    vec4.len === vec4.length &&
      vec4.sub === vec4.subtract &&
      vec4.mul === vec4.multiply &&
      vec4.div === vec4.divide &&
      vec4.dist === vec4.distance &&
      vec4.sqrDist === vec4.squaredDistance &&
      vec4.sqrLen === vec4.squaredLength,
  );
  const packed = new Float32Array([1, 2, 3, 4, 5, 6, 7, 8]);
  check(
    vec4.forEach(
      packed,
      4,
      0,
      0,
      (target, value, amount) => vec4.scale(target, value, amount),
      2,
    ) === packed && valuesClose(packed, [2, 4, 6, 8, 10, 12, 14, 16]),
  );

  console.log(`gl-matrix-vec4:${passed}`);
}

export function runMat3Contract(mat3) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`mat3 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const a = mat3.fromValues(1, 2, 3, 0, 1, 4, 5, 6, 0);
  const b = mat3.fromValues(2, 0, 0, 0, 2, 0, 0, 0, 2);
  const out = mat3.create();

  check(valuesClose(out, [1, 0, 0, 0, 1, 0, 0, 0, 1]));
  check(valuesClose(mat3.clone(a), [1, 2, 3, 0, 1, 4, 5, 6, 0]));
  check(mat3.copy(out, b) === out && valuesClose(out, [2, 0, 0, 0, 2, 0, 0, 0, 2]));
  check(mat3.identity(out) === out && valuesClose(out, [1, 0, 0, 0, 1, 0, 0, 0, 1]));
  check(
    mat3.set(out, 9, 8, 7, 6, 5, 4, 3, 2, 1) === out &&
      valuesClose(out, [9, 8, 7, 6, 5, 4, 3, 2, 1]),
  );
  check(
    valuesClose(
      mat3.fromMat4(
        out,
        new Float32Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
      ),
      [1, 2, 3, 5, 6, 7, 9, 10, 11],
    ),
  );
  check(valuesClose(mat3.transpose(out, b), [2, 0, 0, 0, 2, 0, 0, 0, 2]));
  const selfTranspose = mat3.clone(mat3.fromValues(1, 2, 3, 4, 5, 6, 7, 8, 9));
  check(
    mat3.transpose(selfTranspose, selfTranspose) === selfTranspose &&
      valuesClose(selfTranspose, [1, 4, 7, 2, 5, 8, 3, 6, 9]),
  );
  check(valuesClose(mat3.invert(out, a), [-24, 18, 5, 20, -15, -4, -5, 4, 1]));
  check(mat3.invert(out, mat3.fromValues(1, 2, 3, 2, 4, 6, 3, 6, 9)) === null);
  check(valuesClose(mat3.adjoint(out, a), [-24, 18, 5, 20, -15, -4, -5, 4, 1]));
  check(mat3.determinant(a) === 1);
  check(valuesClose(mat3.multiply(out, a, b), [2, 4, 6, 0, 2, 8, 10, 12, 0]));
  check(
    valuesClose(mat3.translate(out, mat3.create(), [2, 3]), [1, 0, 0, 0, 1, 0, 2, 3, 1]),
  );
  check(
    valuesClose(
      mat3.rotate(out, mat3.create(), Math.PI / 2),
      [0, 1, 0, -1, 0, 0, 0, 0, 1],
    ),
  );
  check(valuesClose(mat3.scale(out, mat3.create(), [2, 3]), [2, 0, 0, 0, 3, 0, 0, 0, 1]));
  check(
    valuesClose(mat3.fromTranslation(out, [2, 3]), [1, 0, 0, 0, 1, 0, 2, 3, 1]),
  );
  check(
    valuesClose(mat3.fromRotation(out, Math.PI / 2), [0, 1, 0, -1, 0, 0, 0, 0, 1]),
  );
  check(valuesClose(mat3.fromScaling(out, [2, 3]), [2, 0, 0, 0, 3, 0, 0, 0, 1]));
  check(
    valuesClose(
      mat3.fromMat2d(out, new Float32Array([1, 2, 3, 4, 5, 6])),
      [1, 2, 0, 3, 4, 0, 5, 6, 1],
    ),
  );
  check(
    valuesClose(
      mat3.fromQuat(out, new Float32Array([0, 0, 0, 1])),
      [1, 0, 0, 0, 1, 0, 0, 0, 1],
    ),
  );
  check(
    valuesClose(
      mat3.normalFromMat4(
        out,
        new Float32Array([2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 1]),
      ),
      [0.5, 0, 0, 0, 1 / 3, 0, 0, 0, 0.25],
    ),
  );
  check(mat3.normalFromMat4(out, new Float32Array(16)) === null);
  check(valuesClose(mat3.projection(out, 2, 4), [1, 0, 0, 0, -0.5, 0, -1, 1, 1]));
  check(mat3.str(b) === "mat3(2, 0, 0, 0, 2, 0, 0, 0, 2)");
  check(close(mat3.frob(b), Math.sqrt(12)));
  check(valuesClose(mat3.add(out, a, b), [3, 2, 3, 0, 3, 4, 5, 6, 2]));
  check(valuesClose(mat3.subtract(out, a, b), [-1, 2, 3, 0, -1, 4, 5, 6, -2]));
  check(valuesClose(mat3.multiplyScalar(out, b, 2), [4, 0, 0, 0, 4, 0, 0, 0, 4]));
  check(
    valuesClose(mat3.multiplyScalarAndAdd(out, a, b, 2), [5, 2, 3, 0, 5, 4, 5, 6, 4]),
  );
  check(mat3.exactEquals(a, mat3.fromValues(1, 2, 3, 0, 1, 4, 5, 6, 0)));
  check(
    mat3.equals(
      a,
      mat3.fromValues(
        1.000001,
        2.000001,
        3.000001,
        0,
        1.000001,
        4.000001,
        5.000001,
        6.000001,
        0,
      ),
    ),
  );
  check(mat3.mul === mat3.multiply && mat3.sub === mat3.subtract);

  console.log(`gl-matrix-mat3:${passed}`);
}

export function runMat4Contract(mat4) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`mat4 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const identity = mat4.create();
  const translated = mat4.fromValues(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1);
  const scaled = mat4.fromValues(2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 1, 2, 3, 1);
  const out = mat4.create();
  const q = new Float32Array([0, 0, Math.sin(Math.PI / 4), Math.cos(Math.PI / 4)]);

  check(valuesClose(identity, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]));
  check(valuesClose(mat4.clone(translated), [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1]));
  check(
    mat4.copy(out, scaled) === out &&
      valuesClose(out, [2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 1, 2, 3, 1]),
  );
  check(
    mat4.fromValues(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16) instanceof
      Float32Array,
  );
  check(
    mat4.set(out, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1) === out &&
      valuesClose(out, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1]),
  );
  check(mat4.identity(out) === out && valuesClose(out, identity));
  check(
    valuesClose(mat4.transpose(out, scaled), [2, 0, 0, 1, 0, 2, 0, 2, 0, 0, 2, 3, 0, 0, 0, 1]),
  );
  const selfTranspose = mat4.clone(scaled);
  check(
    mat4.transpose(selfTranspose, selfTranspose) === selfTranspose &&
      valuesClose(selfTranspose, [2, 0, 0, 1, 0, 2, 0, 2, 0, 0, 2, 3, 0, 0, 0, 1]),
  );
  check(
    valuesClose(mat4.invert(out, translated), [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -1, -2, -3, 1]),
  );
  check(
    mat4.invert(
      out,
      mat4.fromValues(1, 2, 3, 4, 2, 4, 6, 8, 3, 6, 9, 12, 4, 8, 12, 16),
    ) === null,
  );
  check(
    valuesClose(mat4.adjoint(out, translated), [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -1, -2, -3, 1]),
  );
  check(mat4.determinant(translated) === 1);
  check(
    valuesClose(
      mat4.multiply(out, mat4.fromTranslation(mat4.create(), [1, 2, 3]), mat4.fromScaling(mat4.create(), [2, 2, 2])),
      [2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 1, 2, 3, 1],
    ),
  );
  check(valuesClose(mat4.translate(out, identity, [1, 2, 3]), translated));
  check(
    valuesClose(mat4.scale(out, identity, [2, 3, 4]), [2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 1]),
  );
  check(
    valuesClose(mat4.rotate(out, identity, Math.PI / 2, [0, 0, 1]), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
    ]),
  );
  check(mat4.rotate(out, identity, 1, [0, 0, 0]) === null);
  check(
    valuesClose(mat4.rotateX(out, identity, Math.PI / 2), [
      1, 0, 0, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1,
    ]),
  );
  check(
    valuesClose(mat4.rotateY(out, identity, Math.PI / 2), [
      0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1,
    ]),
  );
  check(
    valuesClose(mat4.rotateZ(out, identity, Math.PI / 2), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
    ]),
  );
  check(valuesClose(mat4.fromTranslation(out, [1, 2, 3]), translated));
  check(
    valuesClose(mat4.fromScaling(out, [2, 3, 4]), [2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 1]),
  );
  check(
    valuesClose(mat4.fromRotation(out, Math.PI / 2, [0, 0, 1]), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
    ]),
  );
  check(mat4.fromRotation(out, 1, [0, 0, 0]) === null);
  check(
    valuesClose(mat4.fromXRotation(out, Math.PI / 2), [
      1, 0, 0, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1,
    ]),
  );
  check(
    valuesClose(mat4.fromYRotation(out, Math.PI / 2), [
      0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1,
    ]),
  );
  check(
    valuesClose(mat4.fromZRotation(out, Math.PI / 2), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
    ]),
  );
  check(
    valuesClose(mat4.fromRotationTranslation(out, q, [1, 2, 3]), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1,
    ]),
  );
  const dual = new Float32Array([
    0, 0, Math.sin(Math.PI / 4), Math.cos(Math.PI / 4), 1.06066012, 0.35355338, 1.06066012, -1.06066012,
  ]);
  check(
    valuesClose(mat4.fromQuat2(out, dual), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1,
    ]),
  );
  check(valuesClose(mat4.getTranslation(new Float32Array(3), translated), [1, 2, 3]));
  check(
    valuesClose(
      mat4.getScaling(new Float32Array(3), mat4.fromScaling(mat4.create(), [2, 3, 4])),
      [2, 3, 4],
    ),
  );
  check(
    valuesClose(
      mat4.getRotation(new Float32Array(4), mat4.fromQuat(mat4.create(), q)),
      [0, 0, Math.SQRT1_2, Math.SQRT1_2],
    ),
  );
  check(
    valuesClose(
      mat4.fromRotationTranslationScale(out, q, [1, 2, 3], [2, 2, 2]),
      [0, 2, 0, 0, -2, 0, 0, 0, 0, 0, 2, 0, 1, 2, 3, 1],
    ),
  );
  check(
    valuesClose(
      mat4.fromRotationTranslationScaleOrigin(out, q, [1, 2, 3], [2, 2, 2], [0, 0, 0]),
      [0, 2, 0, 0, -2, 0, 0, 0, 0, 0, 2, 0, 1, 2, 3, 1],
    ),
  );
  check(
    valuesClose(mat4.fromQuat(out, q), [
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
    ]),
  );
  check(
    valuesClose(mat4.frustum(out, -1, 1, -1, 1, 1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1.02020204, -1, 0, 0, -2.02020192, 0,
    ]),
  );
  check(
    valuesClose(mat4.perspectiveNO(out, Math.PI / 2, 1, 0.1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1.002002, -1, 0, 0, -0.2002002, 0,
    ]),
  );
  check(mat4.perspective === mat4.perspectiveNO);
  check(
    valuesClose(mat4.perspective(out, Math.PI / 2, 1, 0.1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1.002002, -1, 0, 0, -0.2002002, 0,
    ]),
  );
  check(
    valuesClose(mat4.perspectiveZO(out, Math.PI / 2, 1, 0.1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1.001001, -1, 0, 0, -0.1001001, 0,
    ]),
  );
  check(
    valuesClose(
      mat4.perspectiveFromFieldOfView(
        out,
        { upDegrees: 45, downDegrees: 45, leftDegrees: 30, rightDegrees: 30 },
        1,
        100,
      ),
      [1.73205078, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1.01010096, -1, 0, 0, -1.01010096, 0],
    ),
  );
  check(
    valuesClose(mat4.orthoNO(out, -1, 1, -1, 1, 0.1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -0.02002002, 0, 0, 0, -1.002002, 1,
    ]),
  );
  check(mat4.ortho === mat4.orthoNO);
  check(
    valuesClose(mat4.ortho(out, -1, 1, -1, 1, 0.1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -0.02002002, 0, 0, 0, -1.002002, 1,
    ]),
  );
  check(
    valuesClose(mat4.orthoZO(out, -1, 1, -1, 1, 0.1, 100), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -0.01001001, 0, 0, 0, -0.001001, 1,
    ]),
  );
  check(
    valuesClose(mat4.lookAt(out, [0, 0, 5], [0, 0, 0], [0, 1, 0]), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, -5, 1,
    ]),
  );
  check(
    valuesClose(mat4.targetTo(out, [0, 0, 5], [0, 0, 0], [0, 1, 0]), [
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 5, 1,
    ]),
  );
  check(
    mat4.str(identity) ===
      "mat4(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)",
  );
  check(close(mat4.frob(scaled), Math.sqrt(27)));
  check(
    valuesClose(
      mat4.add(out, translated, scaled),
      [3, 0, 0, 0, 0, 3, 0, 0, 0, 0, 3, 0, 2, 4, 6, 2],
    ),
  );
  check(
    valuesClose(
      mat4.subtract(out, scaled, translated),
      [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    ),
  );
  check(
    valuesClose(
      mat4.multiplyScalar(out, scaled, 2),
      [4, 0, 0, 0, 0, 4, 0, 0, 0, 0, 4, 0, 2, 4, 6, 2],
    ),
  );
  check(
    valuesClose(
      mat4.multiplyScalarAndAdd(out, translated, scaled, 2),
      [5, 0, 0, 0, 0, 5, 0, 0, 0, 0, 5, 0, 3, 6, 9, 3],
    ),
  );
  check(mat4.exactEquals(translated, mat4.fromValues(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1)));
  check(
    mat4.equals(
      translated,
      mat4.fromValues(1.000001, 0, 0, 0, 0, 1.000001, 0, 0, 0, 0, 1.000001, 0, 1.000001, 2.000001, 3.000001, 1),
    ),
  );
  check(mat4.mul === mat4.multiply && mat4.sub === mat4.subtract);

  console.log(`gl-matrix-mat4:${passed}`);
}

export function runQuatContract(quat) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`quat contract failed at ${passed + 1}`);
    passed += 1;
  };

  const identity = quat.create();
  const a = quat.fromValues(0, 0, 0, 1);
  const b = quat.setAxisAngle(quat.create(), [0, 0, 1], Math.PI / 2);
  const out = quat.create();

  check(valuesClose(identity, [0, 0, 0, 1]));
  check(quat.identity(out) === out && valuesClose(out, [0, 0, 0, 1]));
  check(valuesClose(b, [0, 0, Math.SQRT1_2, Math.SQRT1_2]));
  const axis = new Float32Array(3);
  check(close(quat.getAxisAngle(axis, b), Math.PI / 2) && valuesClose(axis, [0, 0, 1]));
  check(close(quat.getAngle(a, b), Math.PI / 2));
  check(valuesClose(quat.multiply(out, a, b), b));
  check(valuesClose(quat.rotateX(out, a, Math.PI / 2), [Math.SQRT1_2, 0, 0, Math.SQRT1_2]));
  check(valuesClose(quat.rotateY(out, a, Math.PI / 2), [0, Math.SQRT1_2, 0, Math.SQRT1_2]));
  check(valuesClose(quat.rotateZ(out, a, Math.PI / 2), [0, 0, Math.SQRT1_2, Math.SQRT1_2]));
  check(
    valuesClose(quat.calculateW(out, quat.fromValues(0, 0, 0.6, 0)), [0, 0, 0.6, 0.8]),
  );
  check(
    valuesClose(quat.exp(out, quat.fromValues(0, 0, Math.PI / 4, 0)), [
      0, 0, Math.SQRT1_2, Math.SQRT1_2,
    ]),
  );
  check(valuesClose(quat.ln(out, b), [0, 0, Math.PI / 4, 0]));
  check(
    valuesClose(quat.pow(out, b, 0.5), [0, 0, Math.sin(Math.PI / 8), Math.cos(Math.PI / 8)]),
  );
  check(
    valuesClose(quat.slerp(out, a, b, 0.5), [
      0, 0, Math.sin(Math.PI / 8), Math.cos(Math.PI / 8),
    ]),
  );
  withStubbedRandom([0.25, 0.5, 0.75], () => {
    const random = quat.random(out);
    check(random === out && close(quat.length(out), 1));
  });
  check(valuesClose(quat.invert(out, b), [0, 0, -Math.SQRT1_2, Math.SQRT1_2]));
  check(valuesClose(quat.conjugate(out, b), [0, 0, -Math.SQRT1_2, Math.SQRT1_2]));
  check(
    valuesClose(
      quat.fromMat3(out, new Float32Array([0, 1, 0, -1, 0, 0, 0, 0, 1])),
      [0, 0, Math.SQRT1_2, Math.SQRT1_2],
    ),
  );
  check(valuesClose(quat.fromEuler(out, 90, 0, 0), [Math.SQRT1_2, 0, 0, Math.SQRT1_2]));
  check(quat.str(quat.fromValues(1, 2, 3, 4)) === "quat(1, 2, 3, 4)");
  check(valuesClose(quat.clone(b), b));
  check(valuesClose(quat.fromValues(1, 2, 3, 4), [1, 2, 3, 4]));
  check(quat.copy(out, b) === out && valuesClose(out, b));
  check(quat.set(out, 1, 2, 3, 4) === out && valuesClose(out, [1, 2, 3, 4]));
  check(valuesClose(quat.add(out, b, b), [0, 0, Math.SQRT2, Math.SQRT2]));
  check(quat.mul === quat.multiply);
  check(valuesClose(quat.scale(out, b, 2), [0, 0, Math.SQRT2, Math.SQRT2]));
  check(close(quat.dot(b, b), 1));
  check(valuesClose(quat.lerp(out, a, b, 0.5), [0, 0, 0.35355338, 0.85355339]));
  check(close(quat.length(b), 1) && close(quat.squaredLength(b), 1));
  check(quat.len === quat.length && quat.sqrLen === quat.squaredLength);
  check(valuesClose(quat.normalize(out, quat.fromValues(0, 0, 3, 4)), [0, 0, 0.6, 0.8]));
  check(quat.exactEquals(a, quat.fromValues(0, 0, 0, 1)));
  check(quat.equals(a, quat.fromValues(0, 0, 0, 1.000001)));
  check(
    valuesClose(quat.rotationTo(out, [1, 0, 0], [0, 1, 0]), [
      0, 0, Math.SQRT1_2, Math.SQRT1_2,
    ]),
  );
  check(
    valuesClose(
      quat.sqlerp(out, a, b, a, b, 0.5),
      [0, 0, Math.sin(Math.PI / 8), Math.cos(Math.PI / 8)],
    ),
  );
  check(
    valuesClose(quat.setAxes(out, [0, 0, -1], [1, 0, 0], [0, 1, 0]), [0, 0, 0, 1]),
  );

  console.log(`gl-matrix-quat:${passed}`);
}

export function runQuat2Contract(quat2) {
  let passed = 0;
  const check = (condition) => {
    if (!condition) throw new Error(`quat2 contract failed at ${passed + 1}`);
    passed += 1;
  };

  const identity = quat2.create();
  const q = new Float32Array([0, 0, Math.sin(Math.PI / 4), Math.cos(Math.PI / 4)]);
  const a = quat2.fromRotationTranslation(quat2.create(), q, [1, 2, 3]);
  const out = quat2.create();

  check(valuesClose(identity, [0, 0, 0, 1, 0, 0, 0, 0]));
  check(valuesClose(quat2.clone(a), a));
  check(
    valuesClose(quat2.fromValues(1, 2, 3, 4, 5, 6, 7, 8), [1, 2, 3, 4, 5, 6, 7, 8]),
  );
  check(
    valuesClose(
      quat2.fromRotationTranslationValues(
        0,
        0,
        Math.sin(Math.PI / 4),
        Math.cos(Math.PI / 4),
        1,
        2,
        3,
      ),
      a,
    ),
  );
  check(valuesClose(quat2.fromRotationTranslation(out, q, [1, 2, 3]), a));
  check(
    valuesClose(quat2.fromTranslation(out, [1, 2, 3]), [0, 0, 0, 1, 0.5, 1, 1.5, 0]),
  );
  check(valuesClose(quat2.fromRotation(out, q), [0, 0, Math.SQRT1_2, Math.SQRT1_2, 0, 0, 0, 0]));
  check(
    valuesClose(
      quat2.fromMat4(
        out,
        new Float32Array([0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1]),
      ),
      a,
    ),
  );
  check(quat2.copy(out, a) === out && valuesClose(out, a));
  check(quat2.identity(out) === out && valuesClose(out, identity));
  check(
    quat2.set(out, 1, 2, 3, 4, 5, 6, 7, 8) === out &&
      valuesClose(out, [1, 2, 3, 4, 5, 6, 7, 8]),
  );
  check(valuesClose(quat2.getReal(new Float32Array(4), a), [0, 0, Math.SQRT1_2, Math.SQRT1_2]));
  check(
    valuesClose(quat2.getDual(new Float32Array(4), a), [
      1.06066012, 0.35355338, 1.06066012, -1.06066012,
    ]),
  );
  const realDual = quat2.create();
  check(
    quat2.setReal(realDual, q) === realDual &&
      quat2.setDual(realDual, new Float32Array([1, 0, 0, 0])) === realDual &&
      valuesClose(realDual, [0, 0, Math.SQRT1_2, Math.SQRT1_2, 1, 0, 0, 0]),
  );
  check(valuesClose(quat2.getTranslation(new Float32Array(3), a), [1, 2, 3]));
  check(
    valuesClose(quat2.translate(out, a, [1, 0, 0]), [
      0, 0, Math.SQRT1_2, Math.SQRT1_2, 1.41421354, 0.70710677, 1.06066012, -1.06066012,
    ]),
  );
  check(
    valuesClose(quat2.rotateX(out, quat2.fromRotation(quat2.create(), q), Math.PI / 2), [
      0.5, 0.5, 0.5, 0.5, 0, 0, 0, 0,
    ]),
  );
  check(
    valuesClose(quat2.rotateY(out, quat2.fromRotation(quat2.create(), q), Math.PI / 2), [
      -0.5, 0.5, 0.5, 0.5, 0, 0, 0, 0,
    ]),
  );
  check(
    valuesClose(quat2.rotateZ(out, quat2.fromRotation(quat2.create(), q), Math.PI / 2), [
      0, 0, 1, 0, 0, 0, 0, 0,
    ]),
  );
  check(
    valuesClose(
      quat2.rotateByQuatAppend(out, quat2.fromTranslation(quat2.create(), [1, 0, 0]), q),
      [0, 0, Math.SQRT1_2, Math.SQRT1_2, 0.35355338, -0.35355338, 0, 0],
    ),
  );
  check(
    valuesClose(
      quat2.rotateByQuatPrepend(out, q, quat2.fromTranslation(quat2.create(), [1, 0, 0])),
      [0, 0, Math.SQRT1_2, Math.SQRT1_2, 0.35355338, 0.35355338, 0, 0],
    ),
  );
  check(
    valuesClose(
      quat2.rotateAroundAxis(
        out,
        quat2.fromTranslation(quat2.create(), [1, 0, 0]),
        [0, 1, 0],
        Math.PI / 2,
      ),
      [0, Math.SQRT1_2, 0, Math.SQRT1_2, 0.35355338, 0, 0.35355338, 0],
    ),
  );
  check(valuesClose(quat2.add(out, a, a), [
    0, 0, Math.SQRT2, Math.SQRT2, 2.12132025, 0.70710677, 2.12132025, -2.12132025,
  ]));
  check(
    valuesClose(
      quat2.multiply(out, a, quat2.fromTranslation(quat2.create(), [1, 0, 0])),
      [0, 0, Math.SQRT1_2, Math.SQRT1_2, 1.41421354, 0.70710677, 1.06066012, -1.06066012],
    ),
  );
  check(valuesClose(quat2.scale(out, a, 2), [
    0, 0, Math.SQRT2, Math.SQRT2, 2.12132025, 0.70710677, 2.12132025, -2.12132025,
  ]));
  check(close(quat2.dot(a, a), 1));
  check(
    valuesClose(quat2.lerp(out, identity, a, 0.5), [
      0, 0, 0.35355338, 0.85355339, 0.53033006, 0.17677669, 0.53033006, -0.53033006,
    ]),
  );
  check(valuesClose(quat2.invert(out, a), [
    0, 0, -Math.SQRT1_2, Math.SQRT1_2, -1.06066012, -0.35355338, -1.06066012, -1.06066012,
  ]));
  check(valuesClose(quat2.conjugate(out, a), [
    0, 0, -Math.SQRT1_2, Math.SQRT1_2, -1.06066012, -0.35355338, -1.06066012, -1.06066012,
  ]));
  check(close(quat2.length(a), 1) && close(quat2.squaredLength(a), 1));
  check(
    valuesClose(quat2.normalize(out, quat2.scale(quat2.create(), a, 2)), a),
  );
  check(quat2.str(identity) === "quat2(0, 0, 0, 1, 0, 0, 0, 0)");
  check(quat2.exactEquals(a, quat2.clone(a)));
  check(
    quat2.equals(
      a,
      quat2.fromValues(
        0,
        0,
        0.707107,
        0.707107,
        1.060661,
        0.353554,
        1.060661,
        -1.060661,
      ),
    ),
  );
  check(
    quat2.mul === quat2.multiply &&
      quat2.len === quat2.length &&
      quat2.sqrLen === quat2.squaredLength,
  );

  console.log(`gl-matrix-quat2:${passed}`);
}
