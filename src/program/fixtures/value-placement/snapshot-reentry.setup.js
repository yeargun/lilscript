const trace=[];
globalThis.reenter=change=>{trace.push('enter');change();trace.push('leave');};
globalThis.valuePlacementObserve=library=>{trace.push(library.run());console.log(JSON.stringify(trace));};
