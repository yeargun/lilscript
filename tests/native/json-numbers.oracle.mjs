// The same deterministic input sequence; independent ECMAScript conversions.
import {readFileSync} from 'node:fs';
let state=0x31415926,lines=[],length=0;
function word(){let x=state;x^=x<<13;x^=x>>>17;x^=x<<5;return state=x>>>0;}
function line(text){lines.push(text+'\n');length+=text.length+1;if(length>65536){process.stdout.write(lines.join(''));lines=[];length=0;}}
const buffer=Buffer.alloc(8);
function format(bits){buffer.writeBigUInt64LE(bits);line(String(buffer.readDoubleLE()));}
for(let exponent=0n;exponent<2048n;exponent++){
 const bits=exponent<<52n;format(bits);format(bits|0x8000000000000000n);
 if(bits)format(bits-1n);format(bits+1n);
}
for(let i=0;i<100000;i++)format((BigInt(word())<<32n)|BigInt(word()));
for(let i=0;i<100000;i++){
 const mantissa=(BigInt(word())<<32n)|BigInt(word()),exponent=word()%801-400;
 const value=JSON.parse(`${i&1?'-':''}${mantissa}e${exponent}`);
 buffer.writeDoubleLE(value);line(buffer.readBigUInt64LE().toString(16).padStart(16,'0'));
}
for(const text of readFileSync(0,'utf8').split('\n'))if(text){
 buffer.writeDoubleLE(JSON.parse(text));line(buffer.readBigUInt64LE().toString(16).padStart(16,'0'));
}
if(lines.length)process.stdout.write(lines.join(''));
