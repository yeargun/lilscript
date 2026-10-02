// Independent value-copy oracle; arrays remain shared reference members.
const print=console.log;
const payload=value=>({name:value.name,items:value.items});
const original={name:'base'.repeat(2),items:[1,2]};
function change(input){const value=payload(input);value.name+='!';value.items.push(8);return value;}
const copy=change(payload(original));print(original.name);print(copy.name);print(original.items.length);
const saved=payload(original);original.name='new';const found=payload(saved??original);print(found.name);
function optional(value,take){return take?payload(value):null;}
const again=optional(found,true)??payload(original);print(again.name);print(optional(found,false)===null);
const text={value:'generic'.repeat(2),stamp:3},integer={value:42,stamp:4};print(text.value);print(integer.value);
const box={value:payload(found),stamp:7},other={...box,value:payload(box.value)};other.value.name='other';
print(box.value.name);print(other.value.name);
const pair={left:payload(found),right:{...text}};pair.right.value='pair';print(pair.right.value);print(text.value);
const mapped=change(payload(found));print(mapped.name);print(found.items.length);
function unionValue(input){if(typeof input==='string')return input+'?';const copy=payload(input);copy.name+='+';return copy;}
const changed=unionValue(found);print(typeof changed==='string'?changed:changed.name);
const records=Object.assign(Object.create(null),{one:payload(found)});
const recordValue=payload(records.one??original);recordValue.name='local';print(payload(records.one??original).name);print(Object.values(records)[0].name);
const map=new Map([[1,payload(found)]]);print(payload(map.get(1)??original).name);
const array=[payload(found),payload(original)];print((array[9]??null)===null);print(payload(array[0]??original).name);
array[0].name='array';print(array[0].name);print(found.name);
const holder={value:payload(found)};holder.value.name='holder';print(holder.value.name);print(found.name);
const filtered=array.filter(value=>{array[0]=payload(original);return true;}).map(payload);print(filtered[0].name);
print(Array.from({length:2000},(_,i)=>i).reduce((sum,value)=>sum+value,0));
print('products done');
