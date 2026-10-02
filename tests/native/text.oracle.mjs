// Independent ECMAScript operations; no native runtime implementation is reused.
const print=x=>console.log(x);
function casing(text) { print(JSON.stringify(text.toUpperCase())); print(JSON.stringify(text.toLowerCase())); }
function invalid(pattern,flags) { try {print(new RegExp(pattern,flags).test('abc'));}catch(error){print(error.name);} }
casing('Straße İ ﬃ ſ ǰ');
casing("ΟΣ ΟΣΑ ΟΣ' ΟΣ'Α Σ Σ'Α ΆΣ́ ΆΣ́Α");
casing('𐐀𐐨𞤀𞤢😀\uD800x\uDC00');
casing('\u0000\u2028\u2029'); casing('');
const pattern=/a/g;
for(let i=0;i<3;i++){print(pattern.test('baab'));print(pattern.lastIndex);}
pattern.lastIndex=1.9;print(pattern.test('baab'));print(pattern.lastIndex);
for(const index of [-7,Infinity,NaN]){pattern.lastIndex=index;print(pattern.test('a'));print(pattern.lastIndex);}
print(7);const alias=pattern;print(alias===pattern);alias.lastIndex=8;print(pattern.lastIndex);
const sticky=/a/y;print(sticky.test('ba'));print(sticky.lastIndex);
sticky.lastIndex=1;print(sticky.test('ba'));print(sticky.lastIndex);
const plain=/a/;plain.lastIndex=9;print(plain.test('a'));print(plain.lastIndex);
print('ba'.search(plain));print(plain.lastIndex);
pattern.lastIndex=-0;print('ba'.search(pattern));print(1/pattern.lastIndex);
sticky.lastIndex=8;print('ba'.search(sticky));print(sticky.lastIndex);
const empty=new RegExp('','g');print(empty.source);
for(let i=0;i<2;i++){print(empty.test(''));print(empty.lastIndex);}
print(JSON.stringify('😀'.replace(empty,'-')));print(empty.lastIndex);
print(JSON.stringify('😀'.replace(new RegExp('','gu'),'-')));
const meta=new RegExp('a/b\n\r\u2028\u2029','yusmigd');
print(JSON.stringify(meta.source));print(meta.flags);
for(const key of ['global','ignoreCase','multiline','dotAll','sticky','unicode'])print(meta[key]);
print(new RegExp('a/b[+/=]c').source);print(new RegExp('[]/[^/]').source);
print(new RegExp('\\/\\n').source);print(new RegExp('\\\n').source);
print(/^([a-z]+)-\1$/i.test('ab-AB'));
print(/(?<=ab)c(?=d)/.test('zabcd'));
print(/^.$/u.test('😀'));print(/^.$/.test('😀'));
print(/^[\p{Script=Greek}]+$/u.test('ΑβΣ'));
print(/[\p{ASCII}&&\p{Letter}]/v.test('A'));
print(/[\p{ASCII}--[A-Z]]/v.test('A'));print(/[\q{ab|cd}]/v.test('ab'));
print(/^s$/iu.test('ſ'));print(/^a.b$/s.test('a\nb'));print(/^b$/m.test('a\nb\nc'));
print('zabx'.replace(/(a)(b)?/,"$$|$&|$`|$'|$1|$2|$3|$01|$12|$0"));
print('zax'.replace(/(a)(b)?/,'<$1><$2>'));
print('ab ab'.replace(/(?<first>a)(?<second>b)/g,'$<second>$<first>:$<missing>'));
print('b a'.replace(/(?<value>a)|(?<value>b)/g,'<$<value>>'));
print('a'.replace(/a/,'$<value>'));print('a'.replace(/(?<value>a)/,'$<value'));
pattern.lastIndex=99;print('aaa'.replace(pattern,'b'));print(pattern.lastIndex);
plain.lastIndex=99;print('aaa'.replace(plain,'b'));print(plain.lastIndex);
sticky.lastIndex=1;print('aaa'.replace(sticky,'b'));print(sticky.lastIndex);
invalid('\\\n','u');invalid('(','');invalid('a','gg');invalid('a','uv');invalid('a','z');
print('text ownership passed');
