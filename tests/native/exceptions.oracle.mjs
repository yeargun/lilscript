// ECMAScript completions are the independent oracle. Checked indexing adds
// the source language's explicit RangeError obligation before the read.
const print=console.log,describe=v=>typeof v==='string'?v:'runtime';
const explode=()=>{throw 'boom'.repeat(2)},genericFail=v=>{throw v};
function choose(mode){try{if(mode===1)return explode();if(mode===2)throw'old'.repeat(2);return'body'.repeat(2)}catch(e){print(describe(e));return'catch'.repeat(2)}finally{print('finally');if(mode===2)return'override'.repeat(2)}}
function nested(){try{return'outer'.repeat(2)}finally{try{throw'inside'.repeat(2)}catch(e){print(describe(e))}finally{print('inner final')}}}
function displaced(){try{return'original'.repeat(2)}finally{try{try{return'temporary'.repeat(2)}finally{throw'caught override'.repeat(2)}}catch(e){print(describe(e))}}}
function replacements(){try{try{return}finally{throw'replace return'.repeat(2)}}catch(e){print(describe(e))}try{try{throw'discard'.repeat(2)}finally{throw'replacement'.repeat(2)}}catch(e){print(describe(e))}try{genericFail('generic'.repeat(2))}catch(e){print(describe(e))}}
function loops(){for(let i=0;i<4;i++){try{if(i===0)continue;if(i===2)break;print(i)}finally{print(i+10)}}let count=0;for(let i=0;i<3;i++){try{break}finally{count++;if(i<2)continue}}print(count);try{print('inner loop')}finally{for(let i=0;i<3;i++){if(i===1)break;print(i)}}}
const read=(a,i)=>{if(i<0||i>=a.length)throw new RangeError;return a[i]};
function bounds(){const items=[1];for(const fail of [()=>read(items,4),()=>read('a',-1),()=>read('a',-1),()=>'a'.repeat(-1),()=>new Uint8Array(-1),()=>read(new Uint8Array(1),2)])try{print(fail())}catch(e){print(describe(e))}try{throw items}catch(e){if(Array.isArray(e)){print(e===items);e.push(2)}}print(items.length)}
function callbacks(){const source=['a'.repeat(2),'b'.repeat(2),'c'.repeat(2)];for(const fail of [()=>source.map(v=>{if(v==='bb')throw v;return v.repeat(2)}),()=>source.filter(v=>{if(v==='bb')throw v;return true}),()=>source.reduce((acc,v)=>{if(v==='bb')throw acc;return acc+v},'start'),()=>source.every(v=>{throw v}),()=>source.forEach(v=>{throw v})])try{fail()}catch(e){print(describe(e))}}
function captured(){let saved=()=> 'initial';try{explode()}catch(e){saved=()=>describe(e)}print(saved())}
print(choose(0));print(choose(1));print(choose(2));print(nested());print(displaced());replacements();loops();bounds();callbacks();captured();print('boomboom');print('host recovered');print('host raised');print('exceptions done');
