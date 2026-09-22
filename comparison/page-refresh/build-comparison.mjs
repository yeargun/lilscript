// Render current build facts inside the page's existing methodology copy.
import {createHash} from 'node:crypto';
import {existsSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

export const hash = value => createHash('sha256').update(value).digest('hex');
export function sourceFingerprint(root) {
  const files=execFileSync('git',['ls-files','-z','--','src','config','lilscript*.toml','package.json','package-lock.json','scripts/build.mjs','scripts/shared-graph.mjs','tooling','packages'],{cwd:root}).toString().split('\0').filter(Boolean).sort();
  return hash(files.map(path=>`${path}\0${existsSync(join(root,path))?hash(readFileSync(join(root,path))):'MISSING'}\n`).join(''));
}
export function verifyComparison(root, receipt) {
  if (sourceFingerprint(root)!==receipt.publicationSourceFingerprint) throw new Error('Update comparison measurements when source or configuration changes');
  for (const artifact of [...receipt.publicationArtifacts??[],...receipt.comparisonArtifacts??[]]) {
    if (!existsSync(join(root,artifact.path)) || hash(readFileSync(join(root,artifact.path)))!==artifact.sha256) throw new Error(`Comparison artifact changed: ${artifact.path}`);
  }
}
const esc=value=>String(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
const seconds=value=>value==null?'unavailable':`${value.toFixed(value<1?3:2)} s`;
export function renderBuildFacts(data) {
  const timing=data.build;
  const machine=data.machine;
  if (data.sourceBuild) {
    const {original,lilscript}=data.sourceBuild;
    const duration=lane=>lane.complete?`${seconds(lane.medianSeconds)} median (${seconds(lane.minimumSeconds)}–${seconds(lane.maximumSeconds)}; ${lane.samples.length} builds)`:`failed after ${seconds(lane.samples.at(-1)?.wallSeconds)}`;
    const upstream=data.upstream;
    const scope=data.sourceBuild.scopeNote;
    return `<span id="build-comparison"><br><strong>Build time from source.</strong> LilScript package: ${duration(lilscript)}. Original repository: ${duration(original)}.<br><strong>Machine.</strong> Azure ${esc(machine.instanceClass)}, ${esc(machine.cpu)}, ${machine.logicalCpus} vCPUs, ${(machine.memoryBytes/2**30).toFixed(1)} GiB RAM; ${esc(machine.os)}, Node ${esc(machine.node)}.<br>Measured ${esc(data.measuredAt.slice(0,10))} · LilScript <a href="https://github.com/yeargun/lilscript/commit/${esc(data.compiler.commit)}">${esc(data.compiler.commit.slice(0,7))}</a> · original ${esc(upstream.package)} ${esc(upstream.version)} (<a href="${esc(upstream.repository.replace(/\.git$/,''))}/commit/${esc(upstream.commit)}">${esc(upstream.commit.slice(0,7))}</a>).<br>Outputs cleared between builds; installed dependencies reused. Dependency installation is excluded. Native build commands include their own type generation and checks. ${esc(scope)} Shared worker; these wall times are contextual, not a build speedup claim. ${data.comparisonAssembly ? "Original native-toolchain ESM assembly" : "Original comparison ESM assembly"}: ${seconds(timing.originalEsmSeconds)} median, recorded separately. <a href="./source-build.json">Commands, samples and build records ↗</a></span>`;
  }
  const primary=timing.compilerSeconds;
  const lil=primary==null?`LilScript ${data.buildComplete?'package':'build attempt'} ${seconds(timing.packageSeconds)}`:`LilScript compilation ${seconds(primary)}`;
  const totals=primary==null?'':` Package total: ${seconds(timing.packageSeconds)}.`;
  return `<span id="build-comparison"><br><strong>Build time.</strong> ${lil}; original ESM ${seconds(timing.originalSeconds)}.${totals}<br><strong>Machine.</strong> Azure ${esc(machine.instanceClass)}, ${esc(machine.cpu)}, ${machine.logicalCpus} vCPUs, ${(machine.memoryBytes/2**30).toFixed(1)} GiB RAM; ${esc(machine.os)}, Node ${esc(machine.node)}.<br>Measured ${esc(data.measuredAt.slice(0,10))} · LilScript <a href="https://github.com/yeargun/lilscript/commit/${esc(data.compiler.commit)}">${esc(data.compiler.commit.slice(0,7))}</a>. Shared worker; dependencies and tests excluded. The original timing covers its ESM comparison build. <a href="./comparison.json">Build details ↗</a></span>`;
}
export function writeComparison({root,output}) {
  const receipt=JSON.parse(readFileSync(join(root,'comparison/build-receipt.json'),'utf8'));
  verifyComparison(root,receipt);
  const site=existsSync(join(root,'site'))?'site':'web';
  const data=JSON.parse(readFileSync(join(root,site,'comparison.json'),'utf8'));
  const path=join(output,'index.html');
  let html=readFileSync(path,'utf8');
  html=html.replace(/<!-- build-audit:start -->[\s\S]*?<!-- build-audit:end -->\s*/g,'');
  html=html.replace(/<span id="build-comparison">[\s\S]*?<\/span>/g,'');
  const facts=renderBuildFacts(data);
  if (/<div class="method-note">\s*<p>/.test(html)) {
    html=html.replace(/(<div class="method-note">\s*<p>[\s\S]*?)(<\/p>)/,`$1${facts}$2`);
  } else if (/<p class="disclaimer">/.test(html)) {
    html=html.replace(/(<p class="disclaimer">[\s\S]*?)(<\/p>)/,`$1${facts}$2`);
  } else if (/<section class="contract">/.test(html)) {
    html=html.replace(/(<section class="contract">[\s\S]*?<\/div>\s*<p>[\s\S]*?)(<\/p>)/,`$1${facts}$2`);
  } else throw new Error('No existing methodology paragraph for build facts');
  if (!html.includes('id="build-comparison"')) throw new Error('Build facts were not inserted');
  writeFileSync(path,html);
}
if (process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const root=resolve(process.argv[2]??'.');
  verifyComparison(root,JSON.parse(readFileSync(join(root,'comparison/build-receipt.json'),'utf8')));
  console.log('Comparison sources and artifacts match');
}
