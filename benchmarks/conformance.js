const values = [1, 2, 3, 4];
const pushedLength = values.push(5);
const last = values.pop();
const evens = values.filter((value) => ((value % 2) | 0) === 0);
const sum = evens.reduce((total, value) => (total + value) | 0, 0);

evens.forEach((value) => {
  console.log(value);
});

const name = "LilScript";
const includes = name.includes("Script");
const starts = name.startsWith("Lil");
const ends = name.endsWith("Script");
const upper = name.toUpperCase();
const lower = name.toLowerCase();

console.log(`sum=${sum},last=${last},pushed=${pushedLength},len=${values.length}`);
console.log(`checks=${includes},${starts},${ends}`);
console.log(upper);
console.log(lower);
