let replace;
globalThis.keep=value=>{replace=value;};
globalThis.late=()=>{events.push("late");replace();return 5;};
