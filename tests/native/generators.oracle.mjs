const print=x=>console.log(Object.is(x,-0)?'-0':x);let defaults=0;
function choose(){defaults++;print('default');return 2;}function failDefault(){throw 'default failed';}
function* range(count=choose()){print('start');try{for(let i=0;i<count;i++)yield i;}finally{print('range finally');}}
function* badDefault(count=failDefault()){yield count;}
function* delegated(){yield* [7,8];yield* range(2);}
function* decimals(){yield* new Float64Array([1.5,-0]);}
function* twice(value){yield {...value};yield {...value};}
function* callbacks(){for(let i=0;i<3;i++){let local=i;yield ()=>local;}}
function* withCleanup(){try{yield 1;yield 2;}finally{print('cleanup');yield 9;print('cleanup end');}}
function* outer(){try{yield* withCleanup();print('after delegate');}finally{print('outer finally');}}
function* closeThrows(){try{yield 1;}finally{throw 'close failed';}}
function* nested(){try{for(let value of range(3)){if(value===1)continue;yield value;}}finally{print('nested finally');}}
function returnFromLoop(){for(let value of range(3))return 'return '+value;return 'empty';}
let active=null;function* reenter(){try{for(let n of active??range(0))print(n);}catch(error){print(error.name);}yield 42;}
class Sequence{constructor(end){this.end=end;}*values(){for(let i=0;i<this.end;i++)yield i;}}
function run(){
 const unused=range();print(defaults);print('created');for(let value of unused){print(value);break;}for(let value of unused)print('unexpected');
 try{let bad=badDefault();print('missing default error');}catch(error){print(error);}
 for(let value of delegated())print(value);for(let value of decimals())print(value);
 let original={text:'managed'.repeat(2),shared:[3]};const products=twice({...original});original.text='changed';for(let value of products){print(value.text);value.text='local';value.shared.push(4);}print(original.shared.length);
 const saved=[];for(let callback of callbacks())saved.push(callback);for(let callback of saved)print(callback());
 let cleanup=withCleanup();for(let value of cleanup){print(value);break;}for(let value of cleanup)print(value);
 let forwarding=outer();for(let value of forwarding){print(value);break;}for(let value of forwarding)print(value);
 try{for(let value of closeThrows()){try{break;}catch(error){print('wrong inner catch');}}}catch(error){print(error);}
 try{for(let value of closeThrows()){throw 'original throw';}}catch(error){print(error);}
 for(let value of nested()){print(value);break;}print(returnFromLoop());
 active=reenter();for(let value of active??range(0))print(value);active=null;
 const sequence=new Sequence(3);for(let value of sequence.values())print(value);
 const values=new Set([1,2,3]);for(let value of values){print(value);if(value===1){values.delete(2);values.add(4);}if(value===3){values.clear();values.add(5);}}print(values.size);
 for(let i=0;i<100;i++){const stream=twice({...original});for(let value of stream){if(i===99)print(value.text);break;}}
}
run();print('generators done');
