import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";
import { buildSelectedReactivity } from "../scripts/build-reactivity.mjs";
import {
  reactivityExportOwners,
  renderClosedReactivityBuildEntry,
  staticImportedNames,
} from "../src/reactivity/build-entry.mjs";

const root = resolve(import.meta.dirname, "..");

test("static package selection handles aliases, comments, and repeated imports", async () => {
  const source = `
    import { reactive as state, /* retained */ ref } from "vue";
    import { computed, ref as anotherRef } from "vue";
    state; anotherRef; computed;
  `;
  assert.deepEqual(await staticImportedNames(source, "vue"), [
    "computed",
    "reactive",
    "ref",
  ]);
  await assert.rejects(
    staticImportedNames('import * as Vue from "vue"; Vue.ref(1);', "vue"),
    /static named imports/u,
  );
  await assert.rejects(
    staticImportedNames('import("vue");', "vue"),
    /requires static imports/u,
  );
});

test("generated LilScript entry preserves only selected public bindings", () => {
  const owners = reactivityExportOwners(
    'import { ref } from "./ref";\nimport { computed } from "./computed";\n',
  );
  assert.equal(
    renderClosedReactivityBuildEntry(["ref", "computed", "ref"], owners),
    'import { computed } from "./computed";\n' +
      'import { ref } from "./ref";\n' +
      "extern void hostInstallSelected(JsValue value);\n" +
      'hostInstallSelected(JS.object("computed", computed, "ref", ref));\n',
  );
});

test("closed-world project artifact executes the unchanged app surface", async () => {
  const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-selection-test-"));
  try {
    const source = readFileSync(resolve(root, "apps/reactivity/src/main.js"), "utf8");
    const names = await staticImportedNames(source, "vue");
    const output = resolve(temporary, "reactivity.mjs");
    const selected = await buildSelectedReactivity(names, output);
    assert.deepEqual(selected.exportNames, [
      "computed",
      "effect",
      "isReactive",
      "isRef",
      "reactive",
      "ref",
      "stop",
    ]);
    assert.equal(selected.sourceKind, "complete-reactivity");

    const api = await import(`${pathToFileURL(output).href}?test=${Date.now()}`);
    const vue = await import("vue");
    assert.deepEqual(Object.keys(api).sort(), selected.exportNames);
    const exercise = (runtime) => {
      const state = runtime.reactive({ count: 1, items: [2, 4] });
      const multiplier = runtime.ref(3);
      const total = runtime.computed(
        () => state.count * multiplier.value +
          state.items.reduce((sum, value) => sum + value, 0),
      );
      const snapshots = [];
      const runner = runtime.effect(() => snapshots.push(total.value));
      state.count = 2;
      state.items.push(5);
      multiplier.value = 4;
      state.items[0] = 7;
      runtime.stop(runner);
      state.count = 9;
      return {
        snapshots,
        finalTotal: total.value,
        reactive: runtime.isReactive(state),
        ref: runtime.isRef(multiplier),
      };
    };
    assert.deepEqual(exercise(api), exercise(vue));

    const code = readFileSync(output, "utf8");
    assert.match(code, /new WeakMap/u, "selected artifact must include its host storage adapter");
    assert.match(code, /new Proxy/u, "selected artifact must include its host proxy adapter");
    assert.doesNotMatch(code, /EffectScope|Invalid watch source/u);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});
