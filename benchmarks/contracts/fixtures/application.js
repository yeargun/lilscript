let total = 0;
for (let index = 0; index < 64; index++) total = (total + (index * 2 | 0) + Math.imul(index & 3, 3)) | 0;
console.log(total);
