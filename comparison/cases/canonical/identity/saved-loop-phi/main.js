let prev = 0;
let cur = 7;
let count = 0;
while (prev != cur) {
  prev = cur;
  if (cur > 3) {
    cur = cur - 3 | 0;
  } else {
    cur = 0;
  }
  count = count + 1 | 0;
}
console.log(count);
