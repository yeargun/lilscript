let replace;
globalThis.keep=value=>{replace=value;};
globalThis.late=()=>{events.push("late");return 3;};
globalThis.opaque=()=>4;
