import { readFileSync } from "node:fs"

const sourceFixtureNames = ["probe-records", "marked-join-lines", "probe-arithmetic-updates", "probe-captured-updates", "probe-while-updates", "probe-for-control", "probe-for-header", "probe-loop-captures", "probe-nested-loops", "probe-memory", "probe-collections", "probe-exceptions", "probe-protected-branch", "probe-strings", "probe-template-binding", "probe-defaults"]
export const architectureFixtureFiles = ["marked-brackets", "probe-classes", ...sourceFixtureNames]
  .flatMap(name => [`fixtures/${name}.lil`, `fixtures/${name}.origin.json`])

// Closed microprograms for representation experiments, separate from the
// maintained library acceptance matrix. Expected results are declared here.
export function architectureFixtures() {
  const fixtures = [
    { name: "integer-contract", source: `
int unsigned(int value,int shift){return value>>>shift;}
int minimum=-2147483647-1;
print(unsigned(-1,0));print(unsigned(-1,32));print(unsigned(-1,1));
print(minimum/-1);print(1073741825*1073741825);print(7/0);print(-9%2);
`, expected: "-1\n-1\n2147483647\n-2147483648\n-2147483648\n0\n-1\n" },
    { name: "closure-and-control", source: `
int count=0;
bool change(){count=count+1;return true;}
print(false&&change());print(true||change());print(count);
auto add=(int amount)=>{count=count+amount;return count;};
print(add(3));print(add(4));
int sum(int limit){int i=0;int total=0;while(i<limit){i=i+1;if(i==2){continue;}if(i>5){break;}total=total+i;}return total;}
print(sum(7));
`, expected: "false\ntrue\n0\n3\n7\n13\n" },
    { name: "probe-parity", source: `
bool sameParity(int previous,int next){return previous%2==next%2;}
print(sameParity(3,5));print(sameParity(3,4));print(sameParity(-3,3));
`, expected: "true\nfalse\nfalse\n" },
  ]
  for (const [width, depth] of [[8, 8], [64, 16], [128, 32]]) {
    let source = ""
    let total = 0
    for (let fn = 0; fn < width; fn++) {
      source += `int work${fn}(int input){int v0=input;`
      for (let level = 1; level <= depth; level++) source += `int v${level}=v${level - 1}+${fn + 1};`
      source += `int unused=input+1;42;if(false){print(999);}return v${depth};}\n`
      total += 7 + depth * (fn + 1)
    }
    source += "int total=0;\n"
    for (let fn = 0; fn < width; fn++) source += `total=total+work${fn}(7);\n`
    source += "print(total);\n"
    fixtures.push({ name: `value-chains-${width}-${depth}`, source, expected: `${total}\n` })
    // Preserve the closed constant-input case and add an opaque host boundary.
    // The setup is a test fixture, never part of the compiled/minified input.
    fixtures.push({ name: `opaque-value-chains-${width}-${depth}`,
      source: `extern int read();\n${source.replace("int total=0;", "int total=0;int seed=read();").replaceAll("(7);", "(seed);")}`,
      setup: "globalThis.read=()=>7;\n", expected: `${total}\n` })
  }
  let chain = "extern int read();int work(int input){int v0=input;"
  for (let i = 1; i <= 1200; i++) chain += `int v${i}=v${i-1}+1;`
  chain += "return v1200;}print(work(read()));\n"
  fixtures.push({ name: "materialization-chain-1200", source: chain,
    setup: "globalThis.read=()=>7;\n", expected: "1207\n" })
  let shared = "extern int read();int work(int input){int v0=input;"
  for (let i = 1; i <= 72; i++) shared += `int v${i}=v${i-1}+v${i-1};`
  shared += "return v72;}print(work(read()));\n"
  fixtures.push({ name: "shared-value-dependencies-72", source: shared,
    setup: "globalThis.read=()=>7;\n", expected: "0\n" })
  fixtures.push({ name: "generic-callback", source: `extern int read();
bool sameParity(int previous,int next){return previous%2==next%2;}
bool compareWith<T>(T value,func(T,T)->bool compare){return compare(value,value);}
print(compareWith(read(),sameParity));\n`, setup: "globalThis.read=()=>7;\n", expected: "true\n" })
  fixtures.push({ name: "owned-array-length", source: `extern int read();
int work(int seed){int[] values=[seed,seed+1,seed+2,seed+3,seed+4,seed+5,seed+6,seed+7];return values.length+values.length;}
print(work(read()));\n`, setup: "globalThis.read=()=>7;\n", expected: "16\n" })
  fixtures.push({ name: "owned-array-effects", source: `extern int take(int value);
int[] values=[take(1),take(2),take(3)];
print(777);print(values.length);print(values.length);\n`,
    setup: "globalThis.take=value=>{console.log(value);return value};\n", expected: "1\n2\n3\n777\n3\n3\n" })
  fixtures.push({ name: "captured-array-cell", source: `
int[] values=[1,2];auto length=()=>values.length;values=[3,4,5];
print(length());print(values.length);\n`, expected: "3\n3\n" })
  fixtures.push({ name: "array-host-escape", source: `extern void resize(int[] values);
int[] values=[1,2];resize(values);print(values.length);\n`,
    setup: "globalThis.resize=values=>{values.length=0};\n", expected: "0\n" })
  for (const length of [8, 256, 1200]) {
    let source = "int work(){int current=1;"
    for (let index = 0; index < length; index++) source += "current=current+1;"
    source += "return current+1;}print(work());\n"
    fixtures.push({ name: `mutable-value-chain-${length}`, source, expected: `${length + 2}\n` })
  }
  let joined = "extern int read();int work(int input){int current=0;int total=input;"
  let joinedExpected = 7
  for (let index = 0; index < 64; index++) {
    joined += `if(input>${index}){current=${index};}else{current=${index};}total=total+(current+1);`
    joinedExpected += index + 1
  }
  joined += "return total;}print(work(read()));\n"
  fixtures.push({ name: "branch-value-joins-64", source: joined,
    setup: "globalThis.read=()=>7;\n", expected: `${joinedExpected}\n` })
  fixtures.push({ name: "loop-mutable-values", source: `
int looped(int n){int x=2;int i=0;
while(i<n){x=x+1;i=i+1;if(i==2){continue;}if(i==4){break;}print(x+1);}
print(x+1);bool update=n>0&&(x=8)>0;print(x+1);bool keep=n>0||(x=12)>0;return x+1;}
print(looped(0));print(looped(1));print(looped(8));\n`,
    expected: "3\n3\n13\n4\n4\n9\n9\n4\n6\n7\n9\n9\n" })
  fixtures.push({ name: "captured-value-writer", source: `
int outer(int n){int cell=1;auto change=(int value)=>{cell=value;return cell;};
int before=cell+1;int result=change(n)+cell;return before+result+cell;}
print(outer(4));print(outer(7));\n`, expected: "14\n23\n" })
  fixtures.push({ name: "probe-code-unit-boundary", source: `
int codeAt(string text,int index){return text.charCodeAt(index);}
print(codeAt("ab",0));print(codeAt("",0));print(codeAt("ab",-1));
print(codeAt("𝄞",0));print(codeAt("𝄞",1));print(codeAt("𝄞",2));\n`,
    expected: "97\n0\n0\n55348\n56606\n0\n" })
  const bracketOrigin = JSON.parse(readFileSync(new URL("./fixtures/marked-brackets.origin.json", import.meta.url)))
  fixtures.push({ name: "marked-bracket-scanner",
    source: readFileSync(new URL("./fixtures/marked-brackets.lil", import.meta.url), "utf8"),
    setup: `const samples=${JSON.stringify(bracketOrigin.samples)};globalThis.sample=index=>samples[index];\n`,
    expected: bracketOrigin.expected, provenance: bracketOrigin })
  fixtures.push({ name: "typed-string-observations", source: `extern string read();
int work(string text,int index){text.length;text.charAt(index);text.indexOf("ab");return text.charCodeAt(index)+1;}
print(work(read(),1));\n`, setup: 'globalThis.read=()=>"𝄞a";\n', expected: "56607\n" })
  fixtures.push({ name: "primitive-literal-code-units", source: `
print("𝄞".length);print("𝄞".charCodeAt(0));print("𝄞".charCodeAt(1));
print("𝄞".charCodeAt(2));print("".charCodeAt(-1));\n`,
    expected: "2\n55348\n56606\n0\n0\n" })
  fixtures.push({ name: "reflect-closure-assignment", source: `
extern string nameOf(func()->int value);
auto original=()=>1;auto result=(original=()=>2);print(nameOf(result));\n`,
    setup: "globalThis.nameOf=value=>value.name;\n", expected: "original\n",
    contract: { observableFunctionNames: true } })
  fixtures.push({ name: "reflect-callback-alias", source: `
extern string nameOf(func()->int value);
func()->int pass(func()->int veryLongCallbackParameter){auto anotherLongAlias=veryLongCallbackParameter;return anotherLongAlias;}
int declared(){return 7;}
auto original=()=>1;auto result=(original=()=>2);
print(nameOf(pass(result)));print(nameOf(()=>5));print(nameOf(pass(declared)));\n`,
    setup: "globalThis.nameOf=value=>value.name;\n", expected: "original\n\ndeclared\n",
    contract: { observableFunctionNames: true } })
  for (const name of sourceFixtureNames) {
    const origin = JSON.parse(readFileSync(new URL(`./fixtures/${name}.origin.json`, import.meta.url)))
    fixtures.push({ name,
      source: readFileSync(new URL(`./fixtures/${name}.lil`, import.meta.url), "utf8"),
      setup: origin.setup, expected: origin.expected, provenance: origin })
  }
  fixtures.push({ name: "record-reference-effects", source: `
extern Record<int> object();extern string key();extern int replacement();extern string trace();
print(object()[key()]??41);print(object()[key()]=replacement());print(trace());\n`,
    setup: `const events=[];const value=new Proxy(Object.create(null),{
get(target,key){events.push('get');return undefined},
set(target,key,value){events.push('set:'+value);return true}});
globalThis.object=()=>{events.push('object');return value};
globalThis.key=()=>{events.push('key');return 'slot'};
globalThis.replacement=()=>{events.push('value');return 7};
globalThis.trace=()=>events.join(',');\n`,
    expected: "41\n7\nobject,key,get,object,key,value,set:7\n" })
  fixtures.push({ name: "indexed-place-observations", source: `
extern int[] receiver();extern int key();extern int rhs();extern void report();
print(receiver()[key()]+=rhs());report();
print(receiver()[key()]++);report();
print(++receiver()[key()]);report();
`, setup: `let events=[];const data=new Proxy([2147483647],{
get:(a,k)=>{events.push('get:'+k);return a[k]},
set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}});
globalThis.receiver=()=>{events.push('receiver');return data};
globalThis.key=()=>{events.push('key');return 0};
globalThis.rhs=()=>{events.push('rhs');return 1};
globalThis.report=()=>{console.log(events.join(','));events=[]};`,
expected: "-2147483648\nreceiver,key,get:0,rhs,set:0:-2147483648\n-2147483648\nreceiver,key,get:0,set:0:-2147483647\n-2147483646\nreceiver,key,get:0,set:0:-2147483646\n" })
  fixtures.push({ name: "typed-array-update-results", source: `
extern Uint8Array bytes();auto values=bytes();
print(++values[0]);print(values[0]);values[0]=255;print(values[0]++);print(values[0]);
print(values[0]+=257);print(values[0]);
extern Uint32Array words();auto ints=words();print(ints[0]++);print(ints[0]);
extern Float32Array floats();auto fractions=floats();print(++fractions[0]);print(fractions[0]);
`, setup: "globalThis.bytes=()=>new Uint8Array([255]);globalThis.words=()=>new Uint32Array([4294967295]);globalThis.floats=()=>new Float32Array([16777216]);",
expected: "256\n0\n255\n0\n257\n1\n-1\n0\n16777217\n16777216\n" })
  fixtures.push({ name: "constructor-lookup-effects", source: `
extern void install(func()->void mutate);extern void check(func()->void action);
int edge=0;install(()=>{edge=2147483647;});edge=0;
auto value=new ArrayBuffer(edge+1);print(value.byteLength);
check(()=>{new Uint8Array(-1);});
`, setup: `const NativeBuffer=globalThis.ArrayBuffer;
globalThis.install=mutate=>Object.defineProperty(globalThis,'ArrayBuffer',{
configurable:true,get(){console.log('lookup');mutate();
return new Proxy(NativeBuffer,{construct(target,args){console.log(args[0]);return new NativeBuffer(0)}})}});
globalThis.check=action=>{try{action();console.log('missed')}catch(error){console.log(error.name)}};`,
expected: "lookup\n-2147483648\n0\nRangeError\n" })
  fixtures.push({
  "name": "collection-receiver-effects",
  "source": "\n        extern Map<string,int> receiver();extern string key();extern void report();\n        extern void install(func()->void mutate);\n        int edge=0;install(()=>{edge=2147483647;});edge=0;\n        receiver().set(key(),edge+1);report();\n        print(receiver().get(key())==null);report();\n        print(receiver().size);report();\n        receiver().get(\"unused\");report();\n    ",
  "setup": "\n        let events=[],mutate;\n        const collection=new Proxy(Object.create(null),{get(target,name){\n            events.push('get:'+name);\n            if(name==='size')return 4294967295;\n            if(name==='set'){\n                mutate();\n                return function(key,value){events.push('set:'+key+':'+value+':'+(this===collection));return this};\n            }\n            if(name==='get')return function(key){events.push('get:'+key+':'+(this===collection));return undefined};\n            throw Error('unexpected member');\n        }});\n        globalThis.receiver=()=>{events.push('receiver');return collection};\n        globalThis.key=()=>{events.push('key');return 'a'};\n        globalThis.install=action=>{mutate=action};\n        globalThis.report=()=>{console.log(events.join(','));events=[]};\n    ",
  "expected": "receiver,get:set,key,set:a:-2147483648:true\ntrue\nreceiver,get:get,key,get:a:true\n-1\nreceiver,get:size\nreceiver,get:get,get:unused:true\n"
})
  fixtures.push({
  "name": "exception-predecessor-effects",
  "source": "\n        int choose(bool fail){\n            int value=2147483647;\n            try {if(fail){throw \"early\";}value=0;}\n            catch {print(value+1);}\n            finally {print(value+1);}\n            return value+1;\n        }\n        print(choose(true));print(choose(false));\n        extern void fail();\n        int value=2147483647;\n        try {fail();value=0;} catch(auto error){print(error);print(value+1);}\n        finally {print(value+1);}\n        print(value+1);\n        int stopped(){\n            int result=2147483647;\n            try {return result+1;result=0;}\n            finally {print(result+1);}\n        }\n        print(stopped());\n    ",
  "setup": "globalThis.fail=()=>{throw 'foreign'};",
  "expected": "-2147483648\n-2147483648\n-2147483648\n1\n1\nforeign\n-2147483648\n-2147483648\n-2147483648\n-2147483648\n-2147483648\n"
})
  fixtures.push({
  "name": "finally-loop-transfers",
  "source": "\n        int total=0;\n        for(int i=0;i<5;i++){\n            try {\n                if(i==0){continue;}\n                if(i==2){break;}\n                total+=10;\n            } finally {total+=i+1;}\n        }\n        print(total);\n        int visits=0;\n        for(int i=0;i<3;i++){\n            try {break;} finally {visits+=1;if(i<2){continue;}}\n        }\n        print(visits);\n        int stopped=0;\n        for(int i=0;i<3;i++){\n            try {continue;} finally {stopped+=1;break;}\n        }\n        print(stopped);\n        int caught=0;\n        for(int i=0;i<3;i++){\n            try {throw i;}catch{continue;}finally{caught+=1;}\n        }\n        print(caught);\n    ",
  "setup": "",
  "expected": "16\n3\n1\n3\n"
})
  fixtures.push({"name": "template-conversion-effects", "source": "\n        extern JsValue token(func()->void write);\n        extern JsValue symbol();\n        extern int later();\n        extern string trace();\n        int value=0;\n        void write(){value=2147483647;}\n        JsValue delayed=token(write);\n        value=0;\n        print(`${delayed}${value+1}`);\n        value=0;\n        string unused=`${delayed}`;\n        print(value+1);\n        try {print(`${symbol()}${later()}`);}catch{print(\"caught\");}\n        print(trace());\n        ", "setup": "const events=[];globalThis.token=write=>({[Symbol.toPrimitive](hint){events.push(hint);write();if(hint!=='string')throw new Error('wrong hint');return 'converted:'}});globalThis.symbol=()=>Symbol('failure');globalThis.later=()=>{events.push('later');return 9};globalThis.trace=()=>events.join(',');", "expected": "converted:-2147483648\n-2147483648\ncaught\nstring,string\n"})
  fixtures.push({"name": "template-code-unit-boundary", "source": "\n        extern void units(string value);\n        int amount=7;\n        units(`\\ud800${amount}\\udfff`);\n        units(`\\` \\${number} \\\\ \\u{1f600}`);\n        units(`line one\r\nline two${amount}`);\n        units(`\\u0024{notAnIdentifier}${amount}`);\n        units(`a\\\nb${amount}`);\n        ", "setup": "globalThis.units=value=>console.log(Array.from({length:value.length},(_,i)=>value.charCodeAt(i)).join(','));", "expected": "55296,55,57343\n96,32,36,123,110,117,109,98,101,114,125,32,92,32,55357,56832\n108,105,110,101,32,111,110,101,10,108,105,110,101,32,116,119,111,55\n36,123,110,111,116,65,110,73,100,101,110,116,105,102,105,101,114,125,55\n97,98,55\n"})
  fixtures.push({"name": "default-fresh-values", "source": "\n        int withDefaults(int a,int b=10,int c=100){return a+b+c;}\n        auto alias=withDefaults;\n        print(withDefaults(3));print(alias(3,1));print(alias(3,1,2));\n        int value=4;\n        int readDefault(int first,int second=value){return first*10+second;}\n        print(readDefault(value++));\n        auto shadow=()=>{int value=70;return readDefault(2);};print(shadow());\n        int increment(int[] values=[0]){values[0]+=1;return values[0];}\n        print(increment());print(increment());\n    ", "expected": "113\n104\n6\n45\n25\n1\n1\n"})
  fixtures.push({"name": "default-callable-instances", "source": "\n        int offset=2;\n        func(int)->int factory(func(int)->int value=(int n)=>{int saved=n+offset;return saved;}){return value;}\n        auto first=factory();auto second=factory();\n        print(first==second);print(first(2));print(second(4));\n        T identity<T>(T value,func(T)->T transform=(T item)=>item){return transform(value);}\n        print(identity(3));print(identity(\"three\"));\n    ", "expected": "false\n4\n6\n3\nthree\n"})
  fixtures.push({"name": "default-argument-order", "source": "\n        int current=1;\n        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++));\n        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++,current++));\n        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++,current++,current++));\n        print(current);\n    \n\n        int trace=0;\n        int index(){trace=trace*10+1;return 0;}\n        int first(){trace=trace*10+2;return 7;}\n        int second(){trace=trace*10+3;return 8;}\n        print((if(index()==0){\n            (int a,int b,int c=a)=>{print(trace);return c;}\n        }else{\n            (int a,int b,int c=a)=>{print(trace);return c;}\n        })(first(),second()));\n    ", "expected": "111\n233\n456\n7\n123\n7\n"})
  fixtures.push({"name": "default-omission-arity", "source": "\n        extern int tag(JsValue value);\n        extern int arity(func(JsValue,int)->int callable);\n        int choose(JsValue first=7,int second=9){return tag(first)+second;}\n        print(choose());print(choose(JS.undefined()));print(choose(JS.undefined(),1));\n        print(arity(choose));\n    ", "setup": "globalThis.tag=value=>value===undefined?100:Number(value);globalThis.arity=callable=>callable.length;", "expected": "16\n109\n101\n2\n"})
  fixtures.push({"name": "probe-classes", "source": "class Shape {\n  int width;\n  int height;\n\n  init(int width, int height) {\n    this.width = width;\n    this.height = height;\n  }\n\n  int area() {\n    return this.width * this.height;\n  }\n\n  int grow(int by) {\n    this.width += by;\n    return this.width;\n  }\n\n  func()->int areaThunk() {\n    return () => this.width * this.height;\n  }\n}\n\nclass Labelled extends Shape {\n  string label;\n\n  init(int width, int height, string label) {\n    super(width, height);\n    this.label = label;\n  }\n\n  string describe() {\n    return this.label;\n  }\n}\n\nclass Holder {\n  int seed;\n  func(int)->int transform;\n\n  init(int seed, func(int)->int transform) {\n    this.seed = seed;\n    this.transform = transform;\n  }\n\n  int apply(int amount) {\n    func(int)->int local = this.transform;\n    return local(this.seed + amount);\n  }\n}\n\nint classes(int seed) {\n  Shape shape = new Shape(seed, 3);\n  Labelled tagged = new Labelled(seed, 4, \"tag\");\n  Shape viewed = tagged;\n  func()->int thunk = shape.areaThunk();\n  Holder holder = new Holder(seed, (int v) => v ^ 5);\n  int total = shape.area() + shape.grow(2) + thunk();\n  total += tagged.area() + viewed.area() + tagged.describe().length;\n  total += holder.apply(3);\n  return total;\n}\n\nprint(classes(-7));print(classes(-1));print(classes(0));print(classes(3));print(classes(29));print(classes(1000000000));print(classes(-2147483647-1));print(classes(2147483647));\n", "expected": "-101\n3\n17\n59\n483\n-1179869167\n17\n3\n", "provenance": {"schemaVersion": 1, "source": "/home/azureuser/probelil/src/probe.lil", "sourceSha256": "f0280f8b3aff11753fa7a9fa4a6dc268c6f3c1594b56390da0695c2a4bbb47de", "startLine": 114, "endLine": 175, "extractedSha256": "cf8ac003951d9873347a44b40827347e5fe01ef948097e72691efcd215935e7d", "description": "Complete unchanged Shape, Labelled, Holder declarations and classes function from the maintained Probe port."}})
  fixtures.push({"name": "class-reference-order", "source": "\n        class Cell {\n            int value;\n            init(int value) { this.value=value; }\n            int add(int amount) { this.value+=amount;return this.value; }\n            func()->int read() { return ()=>this.value; }\n        }\n        Cell left=new Cell(1);Cell right=new Cell(20);Cell chosen=left;\n        Cell receiver(){print(10);return chosen;}\n        int argument(){chosen=right;print(30);return 3;}\n        print(receiver().add(argument()));print(left.value);print(right.value);\n        func()->int read=left.read();Cell original=left;\n        int replace(){left=right;original.value=40;return 5;}\n        left.value+=replace();print(original.value);print(left.value);print(read());\n        print(original.value++);print(++original.value);\n        ", "expected": "10\n30\n4\n4\n20\n9\n20\n9\n9\n11\n"})
  fixtures.push({"name": "class-initialization-order", "source": "\n        extern int argument();\n        class Base {\n            int value;\n            int[] items;\n            Map<string,int> table;\n            init(int value=2) { this.value=value; }\n            int add(int by=3) { return this.value+by; }\n        }\n        class Derived extends Base {\n            init(int value=4) { super(value);if(value==4){return;}this.value+=1; }\n        }\n        Derived first=new Derived();Derived second=new Derived(argument());\n        first.items.push(9);print(first.items.length);print(second.items.length);\n        first.table.set(\"x\",7);print(first.table.get(\"x\"));print(second.table.get(\"x\"));\n        print(first.add());print(second.add());print(new Base().add());\n        ", "setup": "const NativeMap=Map;globalThis.Map=class extends NativeMap{constructor(){super();console.log('map')}};globalThis.argument=()=>{console.log('argument');return 8};", "expected": "map\nargument\nmap\n1\n0\n7\nnull\n7\n12\nmap\n5\n"})
  fixtures.push({"name": "class-callable-fields", "source": "\n        class Holder<T> {\n            T value;\n            func()->int callback;\n            init(T value,func()->int callback){this.value=value;this.callback=callback;}\n            T get(){return this.value;}\n            U identity<U>(U value){return value;}\n        }\n        Holder<int> first=new Holder<int>(7,()=>1);\n        Holder<string> second=new Holder<string>(\"ok\",()=>3);\n        first.callback=()=>2;auto saved=first.callback;\n        print(first.callback());print(saved());print(second.callback());\n        print(first.get());print(second.get());print(first.identity(\"generic\"));\n        ", "expected": "2\n2\n3\n7\nok\ngeneric\n"})
  fixtures.push({"name": "class-allocation-identity", "source": "\n        class Cell {\n            int value;\n            init(int value) {\n                this.value=value;\n                if(value==1){this=new Cell(2);return;}\n                if(value==4){try{return;}finally{this=new Cell(5);}}\n                if(value==6){auto change=()=>{this=new Cell(7);};change();}\n            }\n            int replace(){this=new Cell(9);return this.value;}\n        }\n        Cell first=new Cell(1);\n        print(first.value);print(new Cell(4).value);print(new Cell(6).value);\n        print(first.replace());print(first.value);\n        ", "expected": "1\n4\n6\n9\n1\n"})
  return fixtures
}
