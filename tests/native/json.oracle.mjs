// Independent ECMAScript parser and formatter, including decimal rounding.
const print=x=>console.log(x);
print(JSON.parse('42'));print(JSON.parse('true'));print(JSON.parse('null')===null);
const text=JSON.parse('"a\\n\\t\\b\\f\\r\\/\\\\\\"\\u0000\\ud800\\uDC00\\ud800x"');
print(JSON.stringify(text));
const values=JSON.parse('[0.1,-1.25,9007199254740993,1e400,-1e-400,5e-324]');
print(JSON.stringify(values));print(1/values[4]);
const integers=JSON.parse('[1,2,3]');integers.push(4);print(JSON.stringify(integers));
const ordered=JSON.parse('{"b":1,"a":2,"b":3,"01":4,"1":5,"__proto__":6}');
print(JSON.stringify(ordered));print(Object.keys(ordered).join(','));print(ordered.__proto__);
const nested=JSON.parse('{"a":[1,2],"b":[],"a":[3,4]}');nested.a.push(5);print(JSON.stringify(nested.a));print(nested.b.length);
print(JSON.parse('[{"name":"first"},{"name":"second"}]')[1].name);
for(const text of ['1.00000000000000011102230246251565404236316680908203125','1.00000000000000011102230246251565404236316680908203126','2.2250738585072011e-308','1.7976931348623157e308','1e-6','1e21'])print(JSON.stringify(JSON.parse(text)));
print(1/JSON.parse('-0'));print(JSON.stringify(NaN));
for(const text of ['', ' ', 'undefined', 'NaN', 'Infinity', '+1', '01', '-', '1.', '1e', '1e+', '.1', 'nullx', 'true false', '[1,]', '{"x":1,}', '{"x" 1}', '[', '{', '[}', '"unterminated', '"\\x20"', '"\\u123Z"', '"\n"', '\u00A01']){
 try{JSON.parse(text);throw new Error('accepted invalid JSON');}catch(error){print(error.name);}
}
JSON.parse('['.repeat(10000)+'null'+']'.repeat(10000));print('deep JSON passed');
print('JSON ownership passed');
