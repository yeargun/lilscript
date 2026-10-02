const print=x=>console.log(x);let defaults=0;
function choose(){defaults++;print('default');return 6;}function failDefault(){throw 'default rejected';}
async function immediate(value=choose()){print('immediate');return value;}
async function invalid(value=failDefault()){return value;}
async function delayed(value){print('delay '+value);await Promise.resolve(0);print('ready '+value);return value;}
async function fail(){await Promise.resolve(0);throw 'failure';}
async function throwingFinally(){try{return 7;}finally{await Promise.resolve(0);throw 'finally rejected';}}
async function returningFinally(){try{throw 'discarded';}finally{await Promise.resolve(0);return 'finally result';}}
async function savedReturn(){try{return 'saved'.repeat(2);}finally{await Promise.resolve(0);print('return finally');}}
async function savedThrow(){try{throw 'saved throw';}finally{await Promise.resolve(0);print('throw finally');}}
async function roundTrip(value){await Promise.resolve(0);return {...value};}
async function makeClosure(){let text='closure'.repeat(2);await Promise.resolve(0);return ()=>text;}
class Counter{constructor(value){this.value=value;}async next(){await Promise.resolve(0);this.value++;return this.value;}}
function* values(){try{yield 1;yield 2;}finally{print('iterable closed');}}
async function loop(){for(let value of values()){await Promise.resolve(value);return 'loop '+value;}return 'empty';}
async function ordering(){
 const one=immediate();print(defaults);print('after call');one.then(value=>print('then '+value));delayed(2).then(value=>print('delayed then '+value));Promise.resolve(0).then(value=>print('tick'));print(await one);print(await delayed(3));
}
async function run(){
 await ordering();
 try{print(await invalid());}catch(error){print(error);}
 try{print(await savedThrow());}catch(error){print(error);}
 print(await savedReturn());print(await returningFinally());
 try{print(await throwingFinally());}catch(error){print(error);}
 print(await loop());
 const recovered=fail().catch(error=>{print(error);return 9;});print(await recovered);
 const chained=Promise.resolve(10).then(value=>delayed(value+1));chained.finally(()=>print('finally chain')).then(value=>print(value));Promise.resolve(0).then(n=>{print('parallel 1');return n;}).then(n=>print('parallel 2'));print(await chained);
 const fin=Promise.resolve(12).finally(()=>delayed(13));print(await fin);
 const rejected=Promise.reject('original');try{print(await rejected.finally(()=>delayed(14)));}catch(error){print(error);}
 try{print(await Promise.resolve(0).finally(()=>fail()));}catch(error){print(error);}
 const tasks=[delayed(2),Promise.resolve(4),delayed(3)];const all=await Promise.all(tasks);print(all.join(','));const empty=await Promise.all([]);print(empty.length);
 const bad=Promise.reject('all rejected');try{let failed=await Promise.all([Promise.resolve(1),bad]);print('missing rejection');}catch(error){print(error);}
 let original={text:'original'.repeat(2),values:[1]};const product=roundTrip({...original});original.text='changed';let first={...await product};first.text='local';first.values.push(2);const second={...await product};print(second.text);print(original.values.length);
 const callback=await makeClosure();print(callback());const counter=new Counter(2);print(await counter.next());
 for(let i=0;i<50;i++){const value=await roundTrip({...original});if(i===49)print(value.text);}print('tasks done');
}
run();print('scheduled');
