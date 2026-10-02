import assert from 'node:assert/strict';
import {readFileSync, mkdtempSync, rmSync} from 'node:fs';
import {createRequire} from 'node:module';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import vm from 'node:vm';
import test from 'node:test';
import {buildSelectedReactivity} from '../scripts/build-reactivity.mjs';

const root=resolve(import.meta.dirname,'..');
const require=createRequire(import.meta.url);
const load=path=>import(pathToFileURL(resolve(root,path)).href);
const publicExports=JSON.parse(readFileSync(resolve(root,'src/package/public-exports.json'),'utf8')).packages;
function reactiveProbe(api) {
  const n=api.ref(2), doubled=api.computed(()=>n.value*2);
  assert.equal(doubled.value,4);
  n.value=3;
  assert.equal(doubled.value,6);
}

test('compiler-written ESM entries share the same package implementation',async()=>{
  const [reactivity,core,vue]=await Promise.all(['reactivity','runtime-core','vue'].map(name=>load(`packages/vuelil/${name}.js`)));
  assert.equal(core.ref,reactivity.ref);
  assert.equal(vue.ref,core.ref);
  assert.notEqual(core.computed,reactivity.computed);
  reactiveProbe(vue);
  assert.equal(vue.version,'3.5.42');
});

test('compiler-written CJS entries share the same package implementation',()=>{
  const reactivity=require('../packages/vuelil/@vue/reactivity/index.js');
  const core=require('../packages/vuelil/@vue/runtime-core/index.js');
  const vue=require('../packages/vuelil/index.cjs');
  assert.equal(core.ref,reactivity.ref);
  assert.equal(vue.ref,core.ref);
  assert.notEqual(core.computed,reactivity.computed);
  reactiveProbe(vue);
  assert.equal(vue.version,'3.5.42');
  const compat=require('../packages/vuelil/@vue/compat/index.js');
  assert.equal(typeof compat.default,'function');
  assert.equal(compat.default.ref,compat.ref);
});

test('browser ESM delivery preserves compilation and public bindings',async()=>{
  const [vue,reactivity,compiler]=await Promise.all(['vue','reactivity','compiler-dom'].map(name=>load(`packages/vuelil/compiler-browser/${name}.js`)));
  assert.equal(vue.ref,reactivity.ref);
  reactiveProbe(vue);
  assert.match(compiler.compile('<div>{{ n }}</div>').code,/return function render/);
  assert.deepEqual(Object.keys(vue).sort(),[...publicExports.vue].sort());
});

test('single-file browser containers retain names, state and template compilation',()=>{
  for(const [path,name,surface] of [
    ['dist/vue.global.js','Vue','vue'],
    ['dist/vue.runtime.global.js','Vue','vue.runtime'],
    ['@vue/reactivity/dist/reactivity.global.js','VueReactivity','reactivity'],
    ['@vue/runtime-dom/dist/runtime-dom.global.js','VueRuntimeDOM','runtime-dom'],
    ['@vue/compiler-dom/dist/compiler-dom.global.js','VueCompilerDOM','compiler-dom'],
  ]) {
    const context=vm.createContext({console,setTimeout,clearTimeout,performance,atob,btoa});
    new vm.Script(readFileSync(resolve(root,'packages/vuelil',path),'utf8'),{filename:path}).runInContext(context);
    const api=context[name];
    assert.deepEqual(Object.keys(api).sort(),[...publicExports[surface]].sort(),path);
    if(api.ref) reactiveProbe(api);
    if(name==='VueCompilerDOM') assert.match(api.compile('<div>{{ n }}</div>').code,/return function render/);
    if(path==='dist/vue.global.js') assert.equal(typeof api.compile('<div>{{ n }}</div>'),'function');
  }
});

test('selected reactivity is a compiler-owned subset with working shared state',async()=>{
  const directory=mkdtempSync(resolve(root,'.tmp/delivery-selected-'));
  try {
    const output=resolve(directory,'selected.mjs');
    const result=await buildSelectedReactivity(['ref','computed'],output);
    const api=await load(output);
    assert.deepEqual(Object.keys(api),['computed','ref']);
    reactiveProbe(api);
    assert.ok(result.manifest.source_sha256);
  } finally {rmSync(directory,{force:true,recursive:true});}
});
