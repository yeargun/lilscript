const trace=[];
globalThis.argument=()=>{trace.push('argument');return 99;};
globalThis.observe=(value,label)=>{trace.push('observe:'+value+':'+label);return label+':'+value;};
globalThis.valuePlacementObserve=library=>{const a=library.make(2),b=library.make(8);trace.push(a(),b(),a());console.log(JSON.stringify(trace));};
