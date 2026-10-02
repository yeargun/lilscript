const first=(values,project)=>project(values[0]);
const repeat=value=>value,other=value=>value,another=value=>value;
const recursive=(value,count)=>count===0?value:recursive(value,count-1),mixed=value=>value;
console.log(first([7,8],value=>value+3));console.log(repeat(4));console.log(other("other"));console.log(another(2.5));
console.log(recursive("recursive",3));console.log(mixed(8));console.log(mixed("mixed"));
let value=11;const old=value;value=13;console.log(old);console.log(value);console.log(value);
