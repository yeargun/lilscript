const descriptor=Object.getOwnPropertyDescriptor(globalThis,'RegExp');
const Native=descriptor.value;
const alias={tag:'alias'};
let trace,chosen,onPattern,onFlags,onLookup,onConstruct;
function First(...args){trace.push(['construct','first',new.target===First,args.length,...args]);if(onConstruct)onConstruct();return alias;}
function Second(...args){trace.push(['construct','second',new.target===Second,args.length,...args]);return alias;}
Object.defineProperty(globalThis,'RegExp',{configurable:true,get(){trace.push('lookup');if(onLookup)onLookup();return chosen;}});
globalThis.pattern=()=>{trace.push('pattern');return onPattern();};
globalThis.flags=()=>{trace.push('flags');return onFlags();};
function run(label,action,configure=()=>{}){
 trace=[];chosen=First;onPattern=()=>'a';onFlags=()=>'g';onLookup=null;onConstruct=null;configure();
 let result;try{const value=action();result=value===alias?'alias':value===undefined?'undefined':value instanceof Native?[value.source,value.flags]:value;}catch(error){result=['throw',error.name,error instanceof SyntaxError||error instanceof TypeError?'native':error.message];}
 events.push([label,trace,result]);
}
try{
 run('one',()=>library.constructOne());
 run('two',()=>library.constructTwo());
 run('explicit-undefined',()=>library.constructTwo(),()=>{onFlags=()=>undefined;});
 run('replace-during-argument',()=>{const first=library.constructTwo();const second=library.constructOne();return first===second?first:null;},()=>{onPattern=()=>{chosen=Second;return 'a';};});
 run('lookup-throws',()=>library.constructTwo(),()=>{onLookup=()=>{throw Error('lookup');};});
 run('pattern-throws',()=>library.constructTwo(),()=>{onPattern=()=>{throw Error('pattern');};});
 run('flags-throws',()=>library.constructTwo(),()=>{onFlags=()=>{throw Error('flags');};});
 run('construct-throws',()=>library.constructTwo(),()=>{onConstruct=()=>{throw Error('construct');};});
 run('arrow-is-not-constructor',()=>library.constructTwo(),()=>{chosen=()=>alias;});
 run('object-is-not-constructor',()=>library.constructOne(),()=>{chosen={};});
 run('native-coercions',()=>library.constructTwo(),()=>{chosen=Native;onPattern=()=>({[Symbol.toPrimitive](hint){trace.push(['coerce-pattern',hint]);return 'x';}});onFlags=()=>({[Symbol.toPrimitive](hint){trace.push(['coerce-flags',hint]);return 'i';}});});
 run('invalid-unused-pattern',()=>library.invalidPattern(),()=>{chosen=Native;});
 run('invalid-unused-flags',()=>library.invalidFlags(),()=>{chosen=Native;});
 run('unused-one',()=>library.unusedOne());
 run('unused-two',()=>library.unusedTwo());
 run('argument-reentry',()=>library.constructTwo(),()=>{let nested=false;onPattern=()=>{if(!nested){nested=true;library.constructOne();nested=false;}return nested?'inner':'outer';};});
}finally{Object.defineProperty(globalThis,'RegExp',descriptor);}
