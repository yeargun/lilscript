function units(value){return Array.from({length:value.length},(_,i)=>value.charCodeAt(i));}
events.push(['trim',library.trimValue('\uFEFF \t\r\nA\u00a0'),library.trimLeft('\uFEFF A \t'),library.trimRight('\t A\u00a0')]);
events.push(['repeat',units(library.repeated('\0\ud800',2)),library.repeated('x',0),library.repeated('😀',2)]);
const trimDescriptor=Object.getOwnPropertyDescriptor(String.prototype,'trim');
const repeatDescriptor=Object.getOwnPropertyDescriptor(String.prototype,'repeat');
let trace,selected,countAction,bodyAction,lookupAction;
const alias={[Symbol.toPrimitive](hint){trace.push(['coerce',hint]);if(bodyAction)bodyAction();return 'raw';}};
globalThis.textValue=()=>{trace.push('text');return ' x ';};
globalThis.repeatCount=()=>{trace.push('count');return countAction();};
function first(...args){trace.push(['call',String(this),args.length,...args]);return alias;}
function later(){throw Error('later-method');}
Object.defineProperty(String.prototype,'repeat',{configurable:true,get(){trace.push('get-repeat');if(lookupAction)lookupAction();return selected;}});
Object.defineProperty(String.prototype,'trim',{configurable:true,get(){trace.push('get-trim');if(lookupAction)lookupAction();return selected;}});
function run(label,action,configure=()=>{}){
 trace=[];selected=first;countAction=()=>2;bodyAction=null;lookupAction=null;configure();let result;
 try{const value=action();result=value===alias?'alias':value===undefined?'undefined':value;}catch(error){result=['throw',error.name,error instanceof RangeError?'native':error.message];}
 events.push([label,trace,result]);
}
try{
 run('raw-repeat',()=>library.repeatCall());
 run('replaced-during-count',()=>library.repeatCall(),()=>{countAction=()=>{selected=later;return 2;};});
 run('getter-throws',()=>library.repeatCall(),()=>{lookupAction=()=>{throw Error('lookup');};});
 run('native-count-coercion',()=>library.repeatCall(),()=>{selected=repeatDescriptor.value;countAction=()=>({valueOf(){trace.push('count-coerce');return 2;}});});
 run('unused-negative-repeat',()=>library.unusedRepeat(),()=>{selected=repeatDescriptor.value;countAction=()=>-1;});
 run('unused-infinite-repeat',()=>library.unusedRepeat(),()=>{selected=repeatDescriptor.value;countAction=()=>Infinity;});
 run('raw-trim',()=>library.trimCall());
 run('trim-coercion',()=>library.coercedTrim());
 run('trim-coercion-throws',()=>library.coercedTrim(),()=>{bodyAction=()=>{throw Error('coercion');};});
 run('unused-trim',()=>library.unusedTrim());
}finally{Object.defineProperty(String.prototype,'trim',trimDescriptor);Object.defineProperty(String.prototype,'repeat',repeatDescriptor);}
