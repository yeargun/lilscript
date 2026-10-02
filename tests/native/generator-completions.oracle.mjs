const print=x=>console.log(x);
function* pendingThrow(){try{throw 'saved throw';}finally{yield 4;print('resumed finally');}}
function* returnWins(){try{throw 'overridden';}finally{yield 5;return;}}
function* empty(value){if(false)yield value;print('empty body');}
function run(){
 try{for(let value of pendingThrow())print(value);}catch(error){print(error);}
 const closed=pendingThrow();for(let value of closed){print(value);break;}for(let value of closed)print('must stay closed');
 for(let value of returnWins())print(value);print('return won');
 const unused=empty('unstarted'.repeat(3));for(let value of empty('started'.repeat(3)))print(value);
 const values=new Set();for(let i=0;i<20;i++)values.add(i);let sum=0;
 for(let value of values){if(value===0){for(let inner of values){if(inner<10)values.delete(inner);if(inner===10){values.add(20);break;}}}sum+=value;}
 print(sum);print(values.size);values.clear();values.add(30);for(let value of values)print(value);
}
run();print('completion done');
