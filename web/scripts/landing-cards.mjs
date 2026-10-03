// Render public measurements from the verified per-library evidence index.
import {readFileSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const data = JSON.parse(readFileSync(join(web, 'src/library-releases.json'), 'utf8'));
const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const number = value => Number(value).toLocaleString('en-US');
const seconds = value => Number(value).toFixed(value < 1 ? 3 : 2);
const codecs = ['raw', 'gzip', 'brotli'];
const labels = {raw:'Raw', gzip:'gzip-9', brotli:'Brotli-11'};
export const rate = (original, lilscript) => `${lilscript > original ? '+' : lilscript < original ? '−' : ''}${Math.abs((lilscript / original - 1) * 100).toFixed(1)}%`;
function pair(row, codec) {
  const item = row.objectives.find(item => item.objective === codec);
  const kind = item.lilscriptBytes < item.originalBytes ? 'win' : item.lilscriptBytes > item.originalBytes ? 'loss' : 'hold';
  return `<div class="${kind} compression-rate" data-compression-rate data-baseline="${item.originalBytes}" data-candidate="${item.lilscriptBytes}"><small>${labels[codec]}</small><b>${rate(item.originalBytes,item.lilscriptBytes)}</b><span>${number(item.originalBytes)} → ${number(item.lilscriptBytes)} B</span></div>`;
}
const cards = data.libraries.map(row => `<a class="lib-card" href="${escape(row.homepage)}"><span class="lib-card-kicker">${escape(row.upstream)}</span><h3>${escape(row.title)}</h3><p>${escape(row.scope)}</p><div class="lib-card-metrics compression-rates">${codecs.map(codec=>pair(row,codec)).join('')}</div></a>`).join('\n');
const ports = data.libraries.map(row => `<tr><th scope="row"><a href="${escape(row.homepage)}">${escape(row.title)}</a><small>${escape(row.scope)}</small></th>${codecs.map(codec => {
  const item=row.objectives.find(item=>item.objective===codec);
  return `<td><span>${number(item.originalBytes)} → ${number(item.lilscriptBytes)} B</span><small>${rate(item.originalBytes,item.lilscriptBytes)} · ${escape(item.originalTool)}</small></td>`;
}).join('')}<td><a href="${escape(row.evidence)}">Files &amp; tests ↗</a><small>Measured ${escape(row.measuredAt.slice(0,10))}</small></td></tr>`).join('\n');
const builds = data.libraries.map(row => `<tr><th scope="row"><a href="${escape(row.homepage)}">${escape(row.title)}</a></th>${codecs.map(codec=>{
  const item=row.objectives.find(item=>item.objective===codec);
  return `<td>${item.originalBuildSeconds == null ? 'unrecorded' : seconds(item.originalBuildSeconds)} / ${seconds(item.lilscriptBuildSeconds)} s</td>`;
}).join('')}<td>${escape(row.timingScope)}</td></tr>`).join('\n');
for (const file of ['index.html','compare.html','libraries.html','benchmarks.html','explorer.html','benchmark-detail.html','demos.html']) {
  const target=join(web,file), before=readFileSync(target,'utf8'); let after=before;
  for(const [section,markup] of Object.entries({cards,ports,builds})) {
    after=after.replace(new RegExp(`<!-- landing-${section}:start -->[\\s\\S]*?<!-- landing-${section}:end -->`),`<!-- landing-${section}:start -->\n${markup}\n<!-- landing-${section}:end -->`);
  }
  if (process.argv.includes('--check')) {if(after!==before)throw Error(`${file} is stale; run node scripts/landing-cards.mjs`);}
  else writeFileSync(target,after);
}
console.log(`Verified ${data.libraries.length} library comparisons`);
