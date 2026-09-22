globalThis.mark=value=>{events.push('mark:'+value);if(value===2)throw 2;return value;};
