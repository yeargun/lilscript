const print=(x)=>console.log(x);
const record=(entries)=>Object.assign(Object.create(null),entries);
const importedChild=[3,7], imported=[importedChild,importedChild];
const values=Array.from({length:5},(_,i)=>i*i);
const nested=[values,values,[]], alias=nested[0], arrays=record({left:values,right:values});
// Record keys preserve source insertion order after integer indices.
const ordered=Object.create(null); for(const [k,v] of [["10","ten"],["2","two"],["b","bee"],["a","aye"],["01","one"],["4294967295","last"],["4294967294","index"],["__proto__","own"],["","empty"]])ordered[k]=v;
const item={text:"\ud800\u0000😀",items:values}, boxed={value:{...item}}, boxedArray=[{value:{...item}}], products=[{...item},{...item}], productRecord={first:{...item}},label={name:"label",items:values};
class Base {constructor(x){this.x=x;}} class Derived extends Base {constructor(x,name){super(x);this.name=name;}}
const derived=new Derived(7,"derived"),base=derived;
print(values.join(",")); print(nested[0]===nested[1]); print(alias===values);print(arrays.left===arrays.right);
print(imported[0]===importedChild);print(importedChild[1]);
print(Object.keys(ordered).join("|"));print(Object.values(ordered).join("|"));print(JSON.stringify(ordered));print(Object.hasOwn(ordered,"__proto__"));print(ordered.missing??"missing");print(JSON.stringify(record({})));
print(item.text.length);print(item.text.charCodeAt(0));print(item.text.charCodeAt(1));
print(boxed.value.items===values);print(boxedArray[0].value.text.length);print(products[1].items===values);print(productRecord.first.items===values);
print(label.items===values);print(label.name);print(base===derived);print(base.x);print(derived.name);
print(JSON.stringify([-0,1.5,Infinity,NaN,5e-324]));print(-1);print(0);print(42);
function fresh(){return [1,2];}
const copy=values.slice(1,4);copy[0]=99;print(copy.join(","));print(values[1]);
const shallow=nested.slice(0,2);print(shallow[0]===values);shallow.push([7]);print(shallow.length);
print(fresh()===fresh());let previous=[];for(let i=0;i<3;i++){const local=[4,5];print(local===previous);previous=local;}
print("static data done");
