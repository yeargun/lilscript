let replace;
globalThis.keep=value=>{replace=value;};
globalThis.rhs=()=>{events.push('rhs');replace();return 7;};
