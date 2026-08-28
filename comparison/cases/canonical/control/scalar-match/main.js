function label(value) {
  return value === -1 ? "negative" : value === 0 ? "zero" : "positive";
}

console.log(label(-1));
console.log(label(0));
console.log(label(2));
