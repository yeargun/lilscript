import { performance } from "node:perf_hooks";
import { artifactUrl, checksum, emitResult, nodeEnvironment } from "./common.mjs";

const compiler = await import(artifactUrl());
const templates = [
  `<main :class="{ ready, compact }"><section v-for="item in items" :key="item.id"><button @click="select(item.id)">{{ item.label }}</button></section></main>`,
  `<form @submit.prevent="save"><input v-model.trim="name"><select v-model="kind"><option v-for="option in options" :value="option.value">{{ option.label }}</option></select></form>`,
  `<article><header><slot name="title" :count="items.length" /></header><template v-if="ready"><p v-for="item in items" :key="item.id">{{ item.text }}</p></template><p v-else>Loading</p></article>`,
  `<component :is="view" v-bind="props" @update:model-value="update"><template #default="slotProps"><strong>{{ slotProps.value }}</strong></template></component>`,
  `<ul><li v-for="(value, key, index) in record" :key="key" :data-index="index"><span v-once>{{ key }}</span>{{ value }}</li></ul>`,
  `<div><svg viewBox="0 0 20 20"><circle v-for="point in points" :key="point.id" :cx="point.x" :cy="point.y" r="2" /></svg><p v-show="visible" v-html="content"></p></div>`,
];
const options = { mode: "module", prefixIdentifiers: true, hoistStatic: true };

for (let pass = 0; pass < 8; pass += 1) {
  for (const template of templates) compiler.compile(template, options);
}

const cycles = 160;
const outputs = [];
const started = performance.now();
for (let cycle = 0; cycle < cycles; cycle += 1) {
  for (const template of templates) outputs.push(compiler.compile(template, options).code);
}
const durationMs = performance.now() - started;

emitResult({
  durationMs,
  operations: cycles * templates.length,
  checksum: checksum(outputs.join("\u0000")),
  environment: nodeEnvironment(),
});
