// Execute actual ESM artifacts, then measure those same complete bytes.
// Node codecs here validate selection mechanics, not the fleet's pinned codec gate.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import zlib from 'node:zlib';
const directory = process.argv[2];
const model = JSON.parse(fs.readFileSync(path.join(directory, 'model.json'), 'utf8'));
const importCode = code => import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
function observe(api, seed, throwNotify) {
  const notifications = [], peeks = [], outcomes = [];
  const step = api.make(seed, peek => {
    notifications.push(peek());
    peeks.push(peek);
    if (throwNotify && peeks.length === 2) throw 'host-error';
  });
  for (const [delta, fail] of [[3, false], [1, true], [-5, false]]) {
    try { outcomes.push(['return', step(delta, fail)]); }
    catch (error) { outcomes.push(['throw', error]); }
  }
  const beforeOther = peeks.map(p => p());
  const otherPeeks = [];
  const other = api.make(17, p => otherPeeks.push(p));
  const otherResult = other(2, false);
  const publicState = api.publicState(seed);
  publicState.value = 123;
  return {outcomes, notifications, final_peeks: peeks.map(p => p()),
    same_peek: peeks.every(p => p === peeks[0]),
    independent: peeks.every((p, i) => p() === beforeOther[i]),
    other_result: otherResult, other_final: otherPeeks[0](),
    public: publicState, public_keys: Object.keys(publicState), unrelated: api.unrelated()};
}
const scores = [], winners = {};
let observations = 0;
for (const [index, artifact] of model.artifacts.entries()) {
  const bytes = Buffer.from(artifact.javascript);
  assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'), artifact.sha256);
  const api = await importCode(artifact.javascript);
  assert.deepEqual(Object.keys(api), ['make', 'publicState', 'unrelated']);
  for (const test of model.scenarios) {
    assert.deepEqual(observe(api, test.seed, test.throw_notify), test.expected, JSON.stringify(artifact.trace));
    observations++;
  }
  const sizes = {raw: bytes.length, gzip: zlib.gzipSync(bytes, {level: 9}).length,
    brotli: zlib.brotliCompressSync(bytes, {params: {
      [zlib.constants.BROTLI_PARAM_QUALITY]: 11,
      [zlib.constants.BROTLI_PARAM_LGWIN]: 22,
      [zlib.constants.BROTLI_PARAM_MODE]: zlib.constants.BROTLI_MODE_TEXT,
    }}).length};
  const row = {index, sha256: artifact.sha256, sizes, trace: artifact.trace, naming: artifact.naming, fast: artifact.fast};
  scores.push(row);
  for (const metric of ['raw', 'gzip', 'brotli']) {
    if (!winners[metric] || sizes[metric] < winners[metric].sizes[metric]) winners[metric] = row;
  }
}
for (const metric of ['raw', 'gzip', 'brotli']) {
  assert.equal(winners[metric].sizes[metric], Math.min(...scores.map(s => s.sizes[metric])));
  fs.writeFileSync(path.join(directory, `winner-${metric}.mjs`), model.artifacts[winners[metric].index].javascript);
}
let extraChecks = 0;
for (const item of model.extra_artifacts) {
  const api = await importCode(item.javascript);
  let calls = 0;
  assert.equal(api.effectCaller(() => { calls++; return 9; }), 18);
  assert.equal(calls, 1);
  assert.throws(() => api.effectCaller(() => {throw 'argument-error';}), e => e === 'argument-error');
  extraChecks += 2;
}
const report = {schema: 1, role: model.role, node: process.version, versions: process.versions,
  source_sha256: model.source_sha256, model_sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(directory, 'model.json'))).digest('hex'),
  candidate_states: model.candidate_states, proposals: model.proposals, proposal_cap: model.proposal_cap,
  fast_states: model.fast_states, fast_proposals: model.fast_proposals,
  fast_minima: Object.fromEntries(['raw', 'gzip', 'brotli'].map(metric =>
    [metric, Math.min(...scores.filter(s => s.fast).map(s => s.sizes[metric]))])),
  distinct_artifacts: scores.length, interpreter_observations: model.interpreter_observations,
  javascript_observations: observations, effectful_argument_checks: extraChecks,
  local_edit_reindexed_units: model.local_edit_reindexed_units,
  scalarization_copied_units: model.scalarization_copied_units,
  query_computations: model.query_computations, query_hits: model.query_hits,
  rejections: model.rejections, winners, scores,
  limitations: ['No production compilation-speed claim.', 'No full parser/type-checker or native backend.',
    'Node codecs are not the pinned fleet encoder service.', 'No Terser/Closure or library competitiveness claim.',
    'No general loops/SCC analysis, full type verifier, generic outline pass or hard time/RSS enforcement.',
    'Pure bounded data sharing; moving effectful initialization is not implemented.']};
fs.writeFileSync(path.join(directory, 'report.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({distinct_artifacts: scores.length, javascript_observations: observations, winners}, null, 2));
