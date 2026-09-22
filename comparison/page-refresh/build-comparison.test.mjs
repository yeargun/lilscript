import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { hash, sourceFingerprint, verifyComparison, writeComparison } from './build-comparison.mjs';

test('publication rejects changed source and ESM artifacts; build facts preserve the existing page structure', () => {
  const root=mkdtempSync(join(tmpdir(),'page-comparison-gate-'));
  try {
    for (const dir of ['src','dist','site','_site','comparison']) mkdirSync(join(root,dir));
    execFileSync('git',['init','--quiet'],{cwd:root});
    const source='fn add(a: number, b: number): number { return a + b; }';
    const esm='export const add=(a,b)=>a+b;';
    writeFileSync(join(root,'src/index.lil'),source);
    writeFileSync(join(root,'dist/index.js'),esm);
    writeFileSync(join(root,'site/esm.js'),esm);
    execFileSync('git',['add','src','dist'],{cwd:root});
    const receipt={publicationSourceFingerprint:sourceFingerprint(root),publicationArtifacts:[{path:'dist/index.js',sha256:hash(esm)}],comparisonArtifacts:[{path:'site/esm.js',sha256:hash(esm)}]};
    verifyComparison(root,receipt);
    writeFileSync(join(root,'site/esm.js'),'export const add=(a,b)=>a-b;');
    assert.throws(()=>verifyComparison(root,receipt),/Comparison artifact changed/);
    writeFileSync(join(root,'site/esm.js'),esm);
    writeFileSync(join(root,'dist/index.js'),'changed distribution');
    assert.throws(()=>verifyComparison(root,receipt),/Comparison artifact changed/);
    writeFileSync(join(root,'dist/index.js'),esm);
    writeFileSync(join(root,'src/index.lil'),'changed source');
    assert.throws(()=>verifyComparison(root,receipt),/Update comparison measurements/);
    writeFileSync(join(root,'src/index.lil'),source);
    const data={measuredAt:'2026-09-10T00:00:00Z',compiler:{commit:'a'.repeat(40)},buildComplete:true,build:{compilerSeconds:2,packageSeconds:3,originalSeconds:.25},machine:{instanceClass:'Standard_D16als_v7',cpu:'AMD EPYC',logicalCpus:16,memoryBytes:32*2**30,os:'Ubuntu',node:'v22'}};
    writeFileSync(join(root,'comparison/build-receipt.json'),JSON.stringify(receipt));
    writeFileSync(join(root,'site/comparison.json'),JSON.stringify(data));
    const original='<!doctype html><link rel="stylesheet" href="styles.css"><main>Existing page<div class="method-note"><p>Existing methodology.</p></div></main>';
    writeFileSync(join(root,'_site/index.html'),original);
    writeComparison({root,output:join(root,'_site')});
    writeComparison({root,output:join(root,'_site')});
    const html=readFileSync(join(root,'_site/index.html'),'utf8');
    assert.equal((html.match(/id="build-comparison"/g)||[]).length,1);
    assert.match(html,/Standard_D16als_v7/);
    assert.equal(html.replace(/<span id="build-comparison">[\s\S]*?<\/span>/,''),original);
    assert.doesNotMatch(html,/<style|build-audit|before.*after/);
  } finally { rmSync(root,{recursive:true,force:true}); }
});

test('source build facts distinguish native repository builds, ESM assembly and failed attempts', async () => {
  const {renderBuildFacts}=await import('./build-comparison.mjs');
  const data={measuredAt:'2026-09-10T00:00:00Z',compiler:{commit:'a'.repeat(40)},upstream:{package:'example',version:'1.0',repository:'https://github.com/example/lib.git',commit:'b'.repeat(40)},machine:{instanceClass:'Standard_D16als_v7',cpu:'AMD EPYC',logicalCpus:16,memoryBytes:32*2**30,os:'Ubuntu',node:'v24'},build:{originalEsmSeconds:.2},sourceBuild:{scopeNote:'Same output boundary.',original:{complete:true,medianSeconds:4,minimumSeconds:3,maximumSeconds:5,samples:[{},{},{}]},lilscript:{complete:false,samples:[{wallSeconds:7}]}}};
  const html=renderBuildFacts(data);
  assert.match(html,/LilScript package: failed after 7.00 s/);
  assert.match(html,/Original repository: 4.00 s median/);
  assert.match(html,/Original comparison ESM assembly: 0.200 s/);
  assert.doesNotMatch(html,/<style|before|after.*before/);
});
