const testDescriptor=Object.getOwnPropertyDescriptor(RegExp.prototype,'test');
const execDescriptor=Object.getOwnPropertyDescriptor(RegExp.prototype,'exec');
const replaceDescriptor=Object.getOwnPropertyDescriptor(String.prototype,'replace');
const rx=new RegExp('a','g'),alias={tag:'raw-result'};
let trace,testMethod,execMethod,inputAction,replacementAction,getAction,bodyAction,which;
function first(value){trace.push(['call',which,'first',this===rx,arguments.length,value]);return bodyAction?bodyAction():alias;}
function second(value){trace.push(['call',which,'second',this===rx,arguments.length,value]);return alias;}
Object.defineProperty(first,'call',{get(){throw Error('function-call-property');}});
Object.defineProperty(second,'call',{get(){throw Error('function-call-property');}});
Object.defineProperty(RegExp.prototype,'test',{configurable:true,get(){trace.push('get-test');if(getAction)getAction();return testMethod;}});
Object.defineProperty(RegExp.prototype,'exec',{configurable:true,get(){trace.push('get-exec');if(getAction)getAction();return execMethod;}});
globalThis.regexValue=()=>{trace.push('regex');return rx;};
globalThis.inputValue=()=>{trace.push('input');return inputAction();};
globalThis.replacementValue=()=>{trace.push('replacement');return replacementAction();};
function run(label,action,configure=()=>{}){
 trace=[];which='test';testMethod=first;execMethod=first;inputAction=()=>'a';replacementAction=()=>'-';getAction=null;bodyAction=null;configure();
 let result;try{const value=action();result=value===alias?'alias':value===undefined?'undefined':value;}catch(error){result=['throw',error.name,error instanceof TypeError?'native':error.message];}
 events.push([label,trace,result]);
}
try{
 run('test-raw-result',()=>library.testCall());
 run('exec-raw-alias',()=>library.execCall(),()=>{which='exec';});
 run('test-getter-throws',()=>library.testCall(),()=>{getAction=()=>{throw Error('getter');};});
 run('test-argument-throws',()=>library.testCall(),()=>{inputAction=()=>{throw Error('argument');};});
 run('exec-invocation-throws',()=>library.execCall(),()=>{which='exec';bodyAction=()=>{throw Error('invoke');};});
 run('replace-test-during-argument',()=>library.testCall(),()=>{inputAction=()=>{testMethod=second;return 'a';};});
 run('replace-exec-during-argument',()=>library.execCall(),()=>{which='exec';inputAction=()=>{execMethod=second;return 'a';};});
 run('test-noncallable',()=>library.testCall(),()=>{testMethod={};});
 run('unused-test',()=>library.unusedTest());
 run('unused-exec',()=>library.unusedExec(),()=>{which='exec';});
 run('argument-reentry',()=>library.testCall(),()=>{let nested=false;inputAction=()=>{if(!nested){nested=true;library.testCall();nested=false;}return nested?'inner':'outer';};});
 // A native .test must still dispatch through the object's mutable .exec.
 run('native-test-exec-dispatch',()=>library.testCall(),()=>{testMethod=testDescriptor.value;which='exec';bodyAction=()=>null;});
 // Native String.replace performs Symbol.replace lookup only after source
 // receiver and all arguments. Returned objects remain raw method results.
 Object.defineProperty(String.prototype,'replace',{configurable:true,get(){trace.push('get-replace');return replaceDescriptor.value;}});
 Object.defineProperty(rx,Symbol.replace,{configurable:true,get(){trace.push('get-symbol-replace');if(getAction)getAction();return function(text,replacement){trace.push(['symbol-replace',this===rx,arguments.length,text,replacement]);return bodyAction?bodyAction():alias;};}});
 run('symbol-replace-result',()=>library.replaceCall());
 run('symbol-replace-throws',()=>library.replaceCall(),()=>{bodyAction=()=>{throw Error('replace');};});
 run('symbol-getter-throws',()=>library.replaceCall(),()=>{getAction=()=>{throw Error('symbol');};});
 run('unused-symbol-replace',()=>library.unusedReplace());
}finally{
 Object.defineProperty(RegExp.prototype,'test',testDescriptor);
 Object.defineProperty(RegExp.prototype,'exec',execDescriptor);
 Object.defineProperty(String.prototype,'replace',replaceDescriptor);
}
