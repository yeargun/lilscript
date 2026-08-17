const source = {left:1, right:2, middle:3};
const copy = {...source, right:11};
source.left = 21;
console.log(copy.left ?? 0);
console.log(copy.right ?? 0);
console.log(Object.keys(copy).join(","));
console.log(JSON.stringify(copy));
