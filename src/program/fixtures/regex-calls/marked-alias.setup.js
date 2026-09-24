const NativeRegExp=globalThis.RegExp;
globalThis.fakeIndent={
 set lastIndex(value){events.push(['set-index',value]);},
 get [Symbol.replace](){events.push('get-replace-hook');return function(text,replacement){events.push(['replace-hook',this===fakeIndent,text,replacement]);return 'indented-result';};}
};
let constructors=0;
globalThis.RegExp=function(pattern,flags){
 events.push(['construct',++constructors,arguments.length]);
 return constructors===1?fakeIndent:new NativeRegExp(pattern,flags);
};
