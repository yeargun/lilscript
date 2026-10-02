const escaping=()=>{let state=0;const step=()=>++state;return()=>step();};
const local=state=>{const step=by=>state+=by,alias=step;return alias(2)+step(3);};
const text=value=>{const append=tail=>value+=tail;append("!");return append("?");};
const abrupt=end=>{let sum=0;for(let n=0;n<end;n++){let held="owned".repeat(n+1);const read=()=>held.length;try{sum+=read();if(n===2)continue;if(n===3)break;}finally{sum+=1;}}return sum;};
console.log(local(4));console.log(text("a".repeat(3)));console.log(abrupt(8));
const saved=escaping();console.log(saved());console.log(saved());
const closures=[];for(const value of [2,4,6]){const read=()=>value;console.log(read());closures.push(()=>value);}
for(const read of closures)console.log(read());
for(const value of [1,3,5]){const read=()=>value+1;console.log(read());}
try{throw "caught".repeat(2);}catch(error){const read=()=>error;console.log(read());}
const recursive=n=>n===0?0:n+recursive(n-1);console.log(recursive(5));
let captured=9;const outer=()=>()=>captured;const result=outer();console.log(result());
