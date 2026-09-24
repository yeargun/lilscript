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
