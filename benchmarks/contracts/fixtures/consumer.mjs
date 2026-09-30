import {transform} from "./library.mjs";
let total = 0;
for (let index = 0; index < 64; index++) total = (total + transform(index)) | 0;
console.log(total);
