const trace=[],sentinel={};let answer=7,fail=false;
globalThis.event=value=>{trace.push(value);if(value===2)answer=40;};
globalThis.read=()=>{trace.push('read:'+answer);if(fail)throw sentinel;return answer;};
globalThis.valuePlacementObserve=library=>{trace.push(library.run());answer=7;fail=true;try{library.run();}catch(error){trace.push(error===sentinel);}console.log(JSON.stringify(trace));};
