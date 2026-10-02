// Use the independent runtime's full, version-matched Unicode implementation.
if(process.versions.unicode!=='17.0')throw new Error('Unicode 17.0 oracle required');
let chunks=[],bytes=0;
for(let point=0;point<=0x10ffff;point++) {
 const input=String.fromCodePoint(point);
 for(const text of [input.toLowerCase(),input.toUpperCase()]) {
  const row=Buffer.alloc(1+2*text.length);row[0]=text.length;
  for(let i=0;i<text.length;i++)row.writeUInt16LE(text.charCodeAt(i),1+2*i);
  chunks.push(row);bytes+=row.length;
 }
 if(bytes>65536){process.stdout.write(Buffer.concat(chunks,bytes));chunks=[];bytes=0;}
}
if(bytes)process.stdout.write(Buffer.concat(chunks,bytes));
