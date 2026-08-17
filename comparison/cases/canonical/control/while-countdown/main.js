let n = 8;
let total = 0;
while (n > 0) {
  total = total + n | 0;
  n--;
}
console.log(total);
