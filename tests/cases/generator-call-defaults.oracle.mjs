// Independent ECMAScript defaults and generator creation/iteration semantics.
let calls=0;
function choose(){calls+=1;console.log("default "+calls);return calls;}
function fail(){throw "default error";}
function* scope(n=choose(),saved=arguments){
    console.log("body "+n);
    console.log(saved===arguments);
    const captured=()=>arguments;
    console.log(captured()===saved);
    yield n;
}
function* broken(n=fail()){yield n;}
class Sequence {
    constructor(start){this.start=start;}
    *next(n=choose()){yield this.start+n;}
}
const a=scope(),b=scope();
console.log(scope.length);
console.log("created "+calls);
for(const n of b)console.log(n);
for(const n of a)console.log(n);
try {const unused=broken();console.log("wrong");}
catch(error){console.log(error);}
const sequence=new Sequence(10),method=sequence.next();
console.log("method created "+calls);
sequence.start=20;
for(const n of method)console.log(n);
