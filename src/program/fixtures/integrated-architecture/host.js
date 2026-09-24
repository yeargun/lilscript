const a=library.make(3),b=library.make(10);
events.push(['a',a(2)]);
events.push(['b',b(1)]);
mode='reenter';events.push(['reentered',a(1)]);
mode='throw';try{b(2);}catch(error){events.push(['caught',error.message]);}
events.push(['retained',retained.get(3).read(),retained.get(10).read()]);
retained.get(3).write(8);events.push(['saved',a(1)]);
events.push(['live',library.total,library.liveTotal]);
const first=library.exposed(2),second=library.exposed(2);first.count=7;
events.push(['public',first!==second,Object.getPrototypeOf(first)===null,first.count,second.count]);
events.push(['exports',Object.keys(library).sort()]);

mode='reenter';
events.push(['integrated',library.runIntegrated(30,2,'    one\n\tsecond')]);
events.push(['integrated-live',library.total,retained.get(30).read()]);
const originalExec=Object.getOwnPropertyDescriptor(library.tabRule,'exec');
try {
  for(const value of [0,null,{marker:7}]) {
    Object.defineProperty(library.tabRule,'exec',{configurable:true,value:function(input){events.push(['override',input]);return value;}});
    library.inspectMatch();
  }
} finally {
  if(originalExec)Object.defineProperty(library.tabRule,'exec',originalExec);
  else delete library.tabRule.exec;
}
events.push(['edit',library.editTarget()]);
