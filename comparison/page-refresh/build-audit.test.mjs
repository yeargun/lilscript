import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { hash, sourceFingerprint, verifyAudit, writeAudit } from './build-audit.mjs';

test('publication refuses source or artifact changes and renders an idempotent offline panel', () => {
  const root=mkdtempSync(join(tmpdir(),'page-receipt-gate-'));
  try {
    for (const dir of ['src','dist','site','_site']) mkdirSync(join(root,dir));
    execFileSync('git',['init','--quiet'],{cwd:root});
    writeFileSync(join(root,'src/index.lil'),'fn add(a: number, b: number): number { return a + b; }');
    writeFileSync(join(root,'dist/index.js'),'export const add=(a,b)=>a+b;');
    execFileSync('git',['add','src','dist'],{cwd:root});
    const data={name:'fixture',status:'verified',compiler:{commit:'abc',sha256:'def'},machine:{memoryBytes:16*2**30,instanceClass:'Standard_D16als_v7'},publicationSourceFingerprint:sourceFingerprint(root),publicationArtifacts:[{path:'dist/index.js',sha256:hash(readFileSync(join(root,'dist/index.js')))}]};
    verifyAudit(root,data);
    writeFileSync(join(root,'dist/index.js'),'export const add=(a,b)=>a-b;');
    assert.throws(()=>verifyAudit(root,data),/Artifact drift/);
    writeFileSync(join(root,'dist/index.js'),'export const add=(a,b)=>a+b;');
    writeFileSync(join(root,'src/index.lil'),'changed source');
    assert.throws(()=>verifyAudit(root,data),/Source\/configuration drift/);
    writeFileSync(join(root,'src/index.lil'),'fn add(a: number, b: number): number { return a + b; }');
    writeFileSync(join(root,'site/build-audit.json'),JSON.stringify(data));
    writeFileSync(join(root,'_site/index.html'),'<!doctype html><main>Existing page</main>');
    writeAudit({root,output:join(root,'_site')});
    writeAudit({root,output:join(root,'_site')});
    const html=readFileSync(join(root,'_site/index.html'),'utf8');
    assert.equal((html.match(/id="build-audit"/g)||[]).length,1);
    assert.ok(html.includes('Standard_D16als_v7'));
    assert.ok(html.includes('Existing page'));
  } finally { rmSync(root,{recursive:true,force:true}); }
});
