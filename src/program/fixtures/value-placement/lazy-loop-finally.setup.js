const trace=[],sentinel={};let fail=false;
globalThis.gate=index=>{trace.push('g'+index);return index!==0;};
globalThis.amount=index=>{trace.push('a'+index);if(fail&&index===2)throw sentinel;return 1;};
globalThis.seen=value=>trace.push('finally:'+value);
globalThis.valuePlacementObserve=library=>{trace.push(library.run());fail=true;try{library.run();}catch(error){trace.push(error===sentinel);}console.log(JSON.stringify(trace));};
