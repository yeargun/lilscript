const count=Number(process.env.LILSCRIPT_N2_ITERATIONS??100000)|0;
let result=0;
if(process.argv[2]==="closures") {
  for(let round=0;round<count;round++) {
    let captured=round^73471;
    const calculate=value=>{
      for(let step=0;step<4+(value&7);step++) captured=(Math.imul(captured,33)+step)|0;
      captured=(captured+value)|0;return captured;
    };
    result^=calculate(round)+calculate(3);
  }
} else {
  const values=[1,2,3,4,5,6,7,8];
  const fold=(items,start,combine)=>{let value=start;for(const item of items)value=combine(value,item);return value;};
  for(let round=0;round<count;round++) result^=fold(values,round,(left,right)=>(left+right)|0);
}
console.log(result);
