function classify(value) {
    if (value > 0) return true;
    for (let i = 0; i < 70; i++) console.log(99);
    return false;
}
console.log(classify(1));
console.log(classify(2));
function report(tag, value = null) { console.log(tag); console.log(value); }
for (let i = 0; i < 2; i++) { report(i); report(i, 7); }
let current = 1;
const snapshot = current;
current = 2;
console.log(snapshot);
console.log(current);
