const trace=[];let target;
function reset(){target={};Object.defineProperty(target,'go',{configurable:true,get(){trace.push('get');return function(value){'use strict';const receiver=this===target?'owner':this===undefined?'undefined':'wrong';trace.push('old:'+receiver+':'+value);return receiver;};}});}
globalThis.object=()=>{trace.push('object');return target;};
globalThis.argument=()=>{trace.push('arg');Object.defineProperty(target,'go',{value:()=>{throw Error('replacement callee ran');},configurable:true});return 5;};
globalThis.invoke=action=>{trace.push('invoke');return action();};
globalThis.valuePlacementObserve=library=>{reset();trace.push(library.reference());reset();trace.push(library.value());trace.push(library.snapshot());console.log(JSON.stringify(trace));};
