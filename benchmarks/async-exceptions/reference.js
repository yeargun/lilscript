async function adjust(value) {
  try {
    if (value % 2 === 0) throw value;
    return await Promise.resolve((value + 1) | 0);
  } catch (ignored) {
    return value;
  }
}

console.log(["featured", "limited", "sale"].map((item) => item.toUpperCase()).join(""));
let checksum = 0;
for (let index = 0; index < 200000; index = (index + 1) | 0) {
  checksum = (checksum + (Math.imul(index, 17) ^ (index >>> 3))) | 0;
}
console.log(checksum);
adjust(4).then((value) => console.log(value));
console.log("limited");
console.log("done");
