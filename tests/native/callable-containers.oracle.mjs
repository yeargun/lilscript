// Independent ECMAScript model. Explicit product copies implement source
// value semantics; function/array/object references preserve their identity.
const print=console.log,copy=p=>({text:p.text,shared:p.shared});
const increment=x=>x+1,annotate=p=>{p=copy(p);p.text+='!';p.shared.push(9);return p;};
const useArray=(callbacks,value)=>{const before=callbacks[0];callbacks[0]=x=>x;print(callbacks.length);return before(value);};
const getMapper=m=>m.callback;
const fromRecord=(values,item)=>{const cb=values.first??(x=>x);values.second=cb;return cb(item);};
const nested=(value,outer,inner)=>outer(inner,value),use=(inner,value)=>inner(value);
const constant=value=>()=>value;
const numbers=[increment];
print(useArray(numbers,10));print(numbers[0](10));numbers[0]=increment;
for(let i=0;i<200;i++){const same=numbers[0];numbers[0]=same;}
print(numbers[0]===increment);print(numbers[0](20));
const mapper={callback:increment};print(getMapper(mapper)(30));
const mapperCopy={...mapper};mapperCopy.callback=x=>x+10;
print(getMapper(mapper)(1));print(getMapper(mapperCopy)(1));
const handler={callback:increment};print(handler.callback(2));
const records={first:increment};print(fromRecord(records,40));const second=records.second??increment;
print(second(50));print(second===increment);print(constant(17)(999));
print(nested(60,use,increment));
const original={text:'owned'.repeat(2),shared:[1]},products=[annotate];
const result=useArray(products,copy(original));
print(result.text);print(original.text);print(original.shared.length);
const productMapper={callback:annotate};print(getMapper(productMapper)(copy(original)).text);
print(nested(copy(original),use,annotate).text);
const retained=constant(copy(original));original.text='changed';print(retained(copy(original)).text);
for(let i=0;i<200;i++){const saved=constant(copy(original));if(i===199)print(saved(copy(original)).text);}
print('callable containers done');
