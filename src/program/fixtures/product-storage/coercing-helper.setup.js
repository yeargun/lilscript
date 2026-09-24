let replace,count=0;
globalThis.keep=value=>{replace=value;};
globalThis.opaque=()=>({[Symbol.toPrimitive](hint){events.push('coerce:'+hint+':'+(++count));if(count===3)throw 3;replace();return 4294967297;}});
