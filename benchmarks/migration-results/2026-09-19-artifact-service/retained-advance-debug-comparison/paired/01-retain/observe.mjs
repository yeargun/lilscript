import fs from 'node:fs';import{pathToFileURL}from'node:url';
const events=[];
const retained=new Map();
let mode='normal';
globalThis.emit=(label,value)=>events.push([label,value]);
globalThis.seedEffect=seed=>{events.push(['unused',seed]);return -1;};
globalThis.keep=(seed,read,write)=>{retained.set(seed,{read,write});events.push(['keep',seed]);};
globalThis.visit=(seed,value,label)=>{
  events.push(['visit',seed,value,label]);
  if(mode==='reenter'){
    mode='normal';retained.get(seed).write(40);
    events.push(['reentry',retained.get(seed).read()]);
  }else if(mode==='throw'){
    mode='normal';retained.get(seed).write(50);throw new Error('host');
  }
};
globalThis.caughtEarly=error=>{if(!(error instanceof ReferenceError)||!/before initialization/.test(error.message))throw error;events.push(['early-tdz',1]);};

// Authored observations for the integrated API and portable source segment.
console.log=value=>events.push(['portable',value]);
globalThis.textEvent=(label,value)=>events.push([label,value]);
globalThis.demandSeen=value=>events.push(['match',Array.isArray(value)?[value[0],value.index]:value]);
globalThis.demandFallback=label=>{events.push(['fallback',label]);return 0;};

const library=await import(pathToFileURL(process.argv[1]).href);
const a=library.make(3),b=library.make(10);
events.push(['a',a(2)]);
events.push(['b',b(1)]);
mode='reenter';events.push(['reentered',a(1)]);
mode='throw';try{b(2);}catch(error){events.push(['caught',error.message]);}
events.push(['retained',retained.get(3).read(),retained.get(10).read()]);
retained.get(3).write(8);events.push(['saved',a(1)]);
events.push(['live',library.total,library.liveTotal]);
const first=library.exposed(2),second=library.exposed(2);first.count=7;
events.push(['public',first!==second,Object.getPrototypeOf(first)===null,first.count,second.count]);
events.push(['exports',Object.keys(library).sort()]);

mode='reenter';
events.push(['integrated',library.runIntegrated(30,2,'    one\n\tsecond')]);
events.push(['integrated-live',library.total,retained.get(30).read()]);
const originalExec=Object.getOwnPropertyDescriptor(library.tabRule,'exec');
try {
  for(const value of [0,null,{marker:7}]) {
    Object.defineProperty(library.tabRule,'exec',{configurable:true,value:function(input){events.push(['override',input]);return value;}});
    library.inspectMatch();
  }
} finally {
  if(originalExec)Object.defineProperty(library.tabRule,'exec',originalExec);
  else delete library.tabRule.exec;
}
events.push(['edit',library.editTarget()]);

process.stdout.write(JSON.stringify(events));
