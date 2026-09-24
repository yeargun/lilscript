let replace;
globalThis.keep=value=>{replace=value;};
globalThis.late=()=>{events.push("late");return 3;};
globalThis.opaque=()=>({[Symbol.toPrimitive](hint){events.push("coerce:"+hint);replace();return 4294967297;}});
