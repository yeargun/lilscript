// Portable GitHub Pages receipt renderer and drift gate, copied into each port.
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const hash = bytes => createHash('sha256').update(bytes).digest('hex');
export function sourceFingerprint(root) {
  const files = execFileSync('git', ['ls-files', '-z', '--', 'src', 'config', 'lilscript*.toml', 'package.json', 'package-lock.json', 'scripts/build.mjs', 'tooling', 'packages'], {cwd:root}).toString().split('\0').filter(Boolean).sort();
  return hash(files.map(path => `${path}\0${existsSync(join(root,path)) ? hash(readFileSync(join(root,path))) : 'MISSING'}\n`).join(''));
}
const esc = value => String(value ?? 'unavailable').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
const bytes = value => typeof value === 'number' ? `${value.toLocaleString('en-US')} B` : 'unavailable';
const seconds = value => typeof value === 'number' ? `${value.toFixed(value < 1 ? 3 : 2)} s` : 'unavailable';
export function renderAudit(data) {
  const machine=data.machine ?? {};
  const timing=data.timing ?? {};
  const sizes=data.sizes ?? {};
  const current = sizes.current;
  const original = sizes.upstream;
  const hardware = `${machine.provider ?? 'Azure'} ${machine.instanceClass ?? ''} · ${machine.cpu ?? 'unrecorded CPU'} · ${machine.logicalCpus ?? '?'} logical CPUs · ${(machine.memoryBytes/2**30).toFixed(1)} GiB RAM`;
  const status=data.status==='verified' ? 'Fresh build and repository tests passed' : data.status==='build-only' ? 'Fresh build; behavior gate unavailable' : 'Refresh found an unresolved build or test failure';
  return `<!-- build-audit:start -->
<style>#build-audit{max-width:1160px;margin:32px auto;padding:28px;border:1px solid #b7c8cf;border-radius:16px;background:#f4fafb;color:#162a32;font:15px/1.6 system-ui,sans-serif;box-sizing:border-box}#build-audit h2{font:600 26px/1.2 system-ui;margin:4px 0 12px;color:#162a32}#build-audit h3{font:600 18px system-ui;color:#162a32;margin:24px 0 8px}#build-audit p{color:#29424d;margin:8px 0}#build-audit a{color:#075b77;text-decoration:underline}#build-audit .audit-scroll{overflow:auto}#build-audit table{width:100%;border-collapse:collapse;font:inherit;white-space:normal;background:transparent;color:#162a32}#build-audit th,#build-audit td{text-align:left;padding:10px 12px;border-bottom:1px solid #cfdee3;background:transparent;color:#162a32}#build-audit code{overflow-wrap:anywhere;font-size:12px}#build-audit .audit-status{font-weight:650;color:${data.status==='verified'?'#116640':'#974710'}}#build-audit details{margin-top:14px}#build-audit ul{padding-left:24px}@media(max-width:650px){#build-audit{margin:16px 10px;padding:18px}#build-audit th,#build-audit td{padding:8px 5px}}</style>
<section id="build-audit" aria-labelledby="build-audit-title">
<p>Compiler and comparison refresh · ${esc(data.measuredAt)}</p>
<h2 id="build-audit-title">What changed, and what it cost to build</h2>
<p class="audit-status">${status}</p>
<p>${esc(data.summary)}</p>
<div class="audit-scroll"><table><thead><tr><th>ESM artifact</th><th>Raw</th><th>gzip-9</th><th>Brotli-11</th></tr></thead><tbody>
<tr><th>Previously published LilScript snapshot</th><td>${bytes(sizes.previous?.raw)}</td><td>${bytes(sizes.previous?.gzip9)}</td><td>${bytes(sizes.previous?.brotli11)}</td></tr>
<tr><th>Fresh LilScript ${data.status==='verified'?'':'(diagnostic)'}</th><td>${bytes(current?.raw)}</td><td>${bytes(current?.gzip9)}</td><td>${bytes(current?.brotli11)}</td></tr>
<tr><th>Fresh original comparison bundle</th><td>${bytes(original?.raw)}</td><td>${bytes(original?.gzip9)}</td><td>${bytes(original?.brotli11)}</td></tr>
</tbody></table></div>
<p>${esc(data.comparisonScope)}</p>
<h3>Build machine and elapsed time</h3><p>${esc(hardware)}<br>${esc(machine.os)} · ${esc(machine.kernel)} · Node ${esc(machine.node)} · Rayon ${esc(machine.rayonThreads)} threads.</p><p>${esc(machine.concurrencyNote)}</p>
<div class="audit-scroll"><table><thead><tr><th>Measured build lane</th><th>Wall time</th><th>Scope</th></tr></thead><tbody>
<tr><th>LilScript primary ESM compilation</th><td>${seconds(timing.primaryCompilerSeconds)}</td><td>${esc(timing.primaryScope ?? 'No isolated primary ESM invocation was identified.')}</td></tr>
<tr><th>LilScript complete package ${data.buildPassed?'build':'attempt'}</th><td>${seconds(timing.packageSeconds)}</td><td>${esc(timing.packageScope)}</td></tr>
<tr><th>Original library comparison build</th><td>${seconds(timing.upstreamSeconds)}</td><td>${esc(timing.upstreamScope)}</td></tr>
</tbody></table></div>
<p>${esc(timing.protocol)} The full package and upstream bundle can produce different sets of files; no speedup ratio is inferred from those totals.</p>
<details><summary>Versions, behavior checks, and reproducibility</summary>
<p>Port source: <code>${esc(data.sourceCommit)}</code><br>Compiler: <code>${esc(data.compiler?.commit)}</code> (${esc(data.compiler?.profile)}); binary SHA-256 <code>${esc(data.compiler?.sha256)}</code>.</p>
<p>Upstream pin: ${esc(data.upstream?.package)}@${esc(data.upstream?.pinned)}. Registry latest at audit: ${esc(data.upstream?.latest)}. ${esc(data.upstream?.note ?? 'Pinned behavior remains the compatibility target; this audit does not claim support for newer releases.')}</p>
<p>${esc(data.behavior?.summary)}</p>
<ul>${(data.findings??[]).map(finding=>`<li>${esc(finding)}</li>`).join('')}</ul>
<p>Current runtime timings elsewhere on this page are historical unless explicitly dated by this receipt. Build times above were measured for this refresh.</p>
<p><a href="./build-audit.json">Download the machine-readable receipt</a> · <a href="https://github.com/yeargun/${esc(data.name)}">Repository and build instructions</a></p>
</details></section>
<!-- build-audit:end -->`;
}

export function verifyAudit(root, data) {
  if (!data.compiler?.sha256 || !data.compiler?.commit) throw new Error('Build audit lacks an exact compiler identity');
  if (sourceFingerprint(root)!==data.publicationSourceFingerprint) throw new Error('Source/configuration drift: refresh site/build-audit.json before publishing comparisons');
  for (const artifact of data.publicationArtifacts??[]) {
    if (!existsSync(join(root,artifact.path)) || hash(readFileSync(join(root,artifact.path)))!==artifact.sha256) throw new Error(`Artifact drift: ${artifact.path} differs from the build audit`);
  }
}

export function writeAudit({root,output}) {
  const receipt = join(root,existsSync(join(root,'site'))?'site':'web','build-audit.json');
  const data=JSON.parse(readFileSync(receipt,'utf8'));
  verifyAudit(root,data);
  const path=join(output,'index.html');
  let html=readFileSync(path,'utf8').replace(/<!-- build-audit:start -->[\s\S]*?<!-- build-audit:end -->\s*/g,'');
  const panel=renderAudit(data);
  html=/<main\b[^>]*>/.test(html) ? html.replace(/<main\b[^>]*>/,match=>match+'\n'+panel) : html.replace(/<body\b[^>]*>/,match=>match+'\n'+panel);
  writeFileSync(path,html);
  writeFileSync(join(output,'build-audit.json'),JSON.stringify(data,null,2)+'\n');
}

if (process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const root=resolve(process.argv[2]??'.');
  if (process.argv.includes('--fingerprint')) console.log(sourceFingerprint(root));
  else {
    const directory=existsSync(join(root,'site'))?'site':'web';
    verifyAudit(root,JSON.parse(readFileSync(join(root,directory,'build-audit.json'),'utf8')));
    console.log('Build audit: source and published artifacts match the receipt');
  }
}
