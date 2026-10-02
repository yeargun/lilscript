// Independent source model: explicit locations for references and value copies.
let effects=0;const print=console.log,effect=x=>(effects=effects*10+x,x);
function defaults(a,b,c){if(a==null)a=effect(1);if(b==null)b=effect(2);if(c==null)c=effect(3);return a*100+b*10+c;}
print(136);
print(defaults(null,effect(8),undefined));print(effects);
effects=0;const indirect=defaults;print(indirect(undefined,effect(9),null));print(effects);
effects=0;print(defaults(0,0,0));print(effects);
effects=0;print(defaults());print(effects);
const choose=(fallback,value)=>value??fallback;
print(choose(12,null));print(choose(13,undefined));print(4);print(choose({name:'fallback',value:9},undefined).name);
print(choose('owned'.repeat(2),null));print(7);print(7);print('boxed'.repeat(2));
const numbers=[1.5,2.5,3.5];let calls=0;
print(numbers.findIndex(value=>{calls++;numbers.pop();return value==null;}));print(calls);
const pairs=[{name:'a',value:1},{name:'b',value:2}];print(pairs.findIndex(value=>{pairs.pop();return value==null;}));
const truncated=[1.5,2.5];print(truncated.findIndex((value=9)=>{truncated.pop();return value===9;}));
function declaration(){let value;value='first'.repeat(2);const read=()=>value;value='second'.repeat(2);return read;}
for(let i=0;i<300;i++){const reader=declaration();if(i===299)print(reader());}
print('call transport passed');
