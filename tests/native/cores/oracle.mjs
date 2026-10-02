import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const count=Number(process.env.LILSCRIPT_N2_ITERATIONS)|0;
let sum=0;
if(process.argv[2]==='katex') {
  // Execute the unchanged pinned upstream implementation. These replacements
  // remove Flow-only syntax; both lookup bodies and their data stay unchanged.
  const origin=JSON.parse(readFileSync(new URL('./origin.json',import.meta.url)));
  let source=readFileSync(new URL(origin.katex.oracle_source,import.meta.url),'utf8');
  if(createHash('sha256').update(source).digest('hex')!==origin.katex.sha256)throw Error('KaTeX oracle source identity changed');
  for(const [from,to] of [
    [/type Script = \{[\s\S]*?\n\};/,''],
    ['const scriptData: Array<Script> =','const scriptData ='],
    ['export function scriptFromCodepoint(codepoint: number): ?string {','function scriptFromCodepoint(codepoint) {'],
    ['const allBlocks: Array<number> =','const allBlocks ='],
    ['export function supportedCodepoint(codepoint: number): boolean {','function supportedCodepoint(codepoint) {'],
  ]) {const next=source.replace(from,to);if(next===source)throw Error('upstream source changed');source=next;}
  const {scriptFromCodepoint,supportedCodepoint}=new Function(source+'\nreturn {scriptFromCodepoint,supportedCodepoint};')();
  for(let round=0;round<count;round++) {
    const codepoint=Math.imul(round,104729)&65535;
    const name=scriptFromCodepoint(codepoint)??'';
    sum=(Math.imul(sum,33)+name.length)|0;
    if(supportedCodepoint(codepoint))sum^=codepoint;
  }
  console.log(sum);console.log(scriptFromCodepoint(0x10000)??'absent');console.log(supportedCodepoint(0x10ffff));
} else if(process.argv[2]==='marked') {
  const source=readFileSync(new URL('../../../src/program/fixtures/regex-calls/marked-rules-complete.lil',import.meta.url),'utf8');
  const names=[...source.matchAll(/^export Regex (\w+) = new Regex\(/gm)].map(m=>m[1]);
  const table=new Map(JSON.parse(readFileSync(new URL('../../../src/program/fixtures/integrated-architecture/marked/rules-oracle-table.json',import.meta.url))).map(([name,source,flags])=>[name,new RegExp(source,flags)]));
  const patterns=names.map(name=>{if(!table.has(name))throw Error(name);return table.get(name);});
  const inputs=JSON.parse(readFileSync(new URL('./marked-inputs.json',import.meta.url)));
  for(let round=0;round<count;round++)for(let pattern=0;pattern<patterns.length;pattern++)for(let input=0;input<inputs.length;input++) {
    patterns[pattern].lastIndex=0;const matched=patterns[pattern].test(inputs[input]);
    sum=Math.imul(sum,33)^(matched?pattern+1:input+1);
  }
  console.log(sum);
} else throw Error('expected katex or marked');
