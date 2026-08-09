import {
  deepEqual,
  equal,
  match,
  notEqual,
  ok,
  throws,
} from "node:assert/strict";

const implementations = [
  ["npm", await import("./node_modules/nanoid/index.browser.js")],
  ["lilscript", await import("./apps/nanoid/lil/api.js")],
];

for (const [name, api] of implementations) {
  const { customAlphabet, customRandom, nanoid, random, urlAlphabet } = api;

  equal(random.length, 1, name);
  equal(customRandom.length, 3, name);
  equal(customAlphabet.length, 1, name);
  equal(nanoid.length, 0, name);
  for (const callable of [random, customRandom, customAlphabet, nanoid]) {
    throws(() => new callable(), TypeError, name);
  }

  for (let index = 0; index < 100; index += 1) {
    const id = nanoid();
    equal(id.length, 21, name);
    equal(typeof id, "string", name);
    for (const character of id) match(urlAlphabet, new RegExp(character, "g"));
  }

  equal(nanoid(10).length, 10, name);
  equal(nanoid("10").length, 10, name);
  equal(nanoid(0), "", name);

  const used = new Set();
  for (let index = 0; index < 50_000; index += 1) used.add(nanoid());
  equal(used.size, 50_000, name);

  nanoid(2.1);
  notEqual(nanoid(), nanoid(), name);

  const idFrequencies = new Map();
  const idCount = 100_000;
  const idLength = nanoid().length;
  for (let index = 0; index < idCount; index += 1) {
    for (const character of nanoid()) {
      idFrequencies.set(
        character,
        (idFrequencies.get(character) ?? 0) + 1,
      );
    }
  }
  equal(idFrequencies.size, urlAlphabet.length, name);
  const idDistribution = [...idFrequencies.values()].map(
    (count) => (count * urlAlphabet.length) / (idCount * idLength),
  );
  ok(Math.max(...idDistribution) - Math.min(...idDistribution) <= 0.05, name);

  const single = customAlphabet("a", 5);
  equal(single(), "aaaaa", name);
  equal(customAlphabet("a")(10), "aaaaaaaaaa", name);
  equal(customRandom("a", 0, (size) => new Uint8Array(size))(), "a", name);
  equal(
    customRandom("a", 0, (size) => new Uint8Array(size).fill(1))(),
    "",
    name,
  );

  const alphabet = "abcdefghijklmnopqrstuvwxyz";
  const generated = customAlphabet(alphabet, 30);
  equal(generated.length, 0, name);
  throws(() => new generated(), TypeError, name);
  const frequencies = new Map();
  for (let index = 0; index < 50_000; index += 1) {
    for (const character of generated()) {
      frequencies.set(character, (frequencies.get(character) ?? 0) + 1);
    }
  }
  equal(frequencies.size, alphabet.length, name);
  const distribution = [...frequencies.values()].map(
    (count) => (count * alphabet.length) / (50_000 * 30),
  );
  ok(Math.max(...distribution) - Math.min(...distribution) <= 0.05, name);

  generated(2.1);
  notEqual(generated(), generated(), name);

  const sequence = [2, 255, 3, 7, 7, 7, 7, 7, 0, 1];
  const fakeRandom = (size) => {
    let bytes = [];
    for (let index = 0; index < size; index += sequence.length) {
      bytes = bytes.concat(sequence.slice(0, size - index));
    }
    return bytes;
  };
  equal(customRandom("abcde", 4, fakeRandom)(), "adca", name);
  equal(
    customRandom("abcde", 18, fakeRandom)(),
    "cbadcbadcbadcbadcc",
    name,
  );

  equal(typeof urlAlphabet, "string", name);
  for (let index = 0; index < urlAlphabet.length; index += 1) {
    equal(urlAlphabet.lastIndexOf(urlAlphabet[index]), index, name);
  }

  for (let index = 0; index < urlAlphabet.length; index += 1) {
    equal(random(10).length, 10, name);
  }
  equal(random(2.9).length, 2, name);
  equal(random(Number.NaN).length, 0, name);

  let observedStep = 0;
  const marker = {};
  const overflowStep = customRandom(
    urlAlphabet,
    1363481680.6349206,
    (step) => {
      observedStep = step;
      throw marker;
    },
  );
  try {
    overflowStep(1);
    throw new Error(`${name}: customRandom callback was not called`);
  } catch (error) {
    equal(error, marker, name);
  }
  equal(observedStep, 2147483648, name);

  const bytes = random(1000);
  ok(bytes instanceof Uint8Array, name);
  equal(bytes.length, 1000, name);
  for (const byte of bytes) {
    equal(typeof byte, "number", name);
    ok(byte >= 0 && byte <= 255, name);
  }
}

deepEqual(
  Object.keys(implementations[0][1]).sort(),
  Object.keys(implementations[1][1]).sort(),
);

console.log(`nanoid-upstream:${implementations.length}`);
