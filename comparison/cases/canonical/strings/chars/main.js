const word = "lilscript";
let total = 0;
for (let i = 0; i < word.length; i++) {
  total = total + word.charCodeAt(i) | 0;
}
console.log(total);
console.log(word.toLowerCase());
console.log(word.toUpperCase());
