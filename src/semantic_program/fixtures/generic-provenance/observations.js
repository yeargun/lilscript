events.push(["plain",library.genericScore(2,3),library.genericScore(7,6)]);
let entered=false;
const payload={ [Symbol.toPrimitive](hint){
  events.push(["coerce",hint]);
  if(!entered){entered=true;events.push(["reentry",library.genericScore(4,1)]);}
  return 2;
}};
events.push(["scalar-object",library.genericScalar(payload)===payload]);
events.push(["object",library.genericScore(payload,3)]);
const symbol=Symbol("opaque");
events.push(["scalar-symbol",library.genericScalar(symbol)===symbol]);
try{library.genericScore(symbol,3);events.push(["symbol-returned"]);}catch(error){events.push(["symbol-throw",error instanceof TypeError]);}
events.push(["scalar-bigint",library.genericScalar(1n)===1n]);
try{library.genericScore(1n,3);events.push(["bigint-returned"]);}catch(error){events.push(["bigint-throw",error instanceof TypeError]);}
let conversions=0;
const throwing={ [Symbol.toPrimitive](){events.push(["throwing",++conversions]);if(conversions===2)throw Error("second read");return 2;} };
try{library.genericScore(throwing,3);events.push(["throwing-returned"]);}catch(error){events.push(["caught",error.message]);}
