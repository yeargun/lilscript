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
