import {
  immNull,
  immBool,
  immNumber,
  immString,
  immArray,
  immObject,
  createDraft,
  isArrayDraft,
  isObjectDraft,
  draftModified,
  originalOf,
  currentOf,
  draftLength,
  draftGetProp,
  draftGetIndex,
  draftSetProp,
  draftSetIndex,
  draftDeleteProp,
  draftPush,
  draftPop,
  finishDraft,
  finishDraftWithPatches,
  replacementPatches,
} from "../../../build/immer-lilscript.js";

const drafts = new WeakMap();
const origins = new WeakMap();
let patchesEnabled = false;

function toLil(value) {
  if (value === null) {
    const node = immNull();
    origins.set(node, value);
    return node;
  }
  if (typeof value === "boolean") {
    const node = immBool(value);
    origins.set(node, value);
    return node;
  }
  if (typeof value === "number") {
    const node = immNumber(value);
    origins.set(node, value);
    return node;
  }
  if (typeof value === "string") {
    const node = immString(value);
    origins.set(node, value);
    return node;
  }
  if (Array.isArray(value)) {
    const items = value.map(toLil);
    const node = immArray(items);
    origins.set(node, value);
    return node;
  }
  if (typeof value === "object") {
    const fields = new Map();
    const keys = Object.keys(value);
    for (let i = 0; i < keys.length; i += 1) {
      const key = keys[i];
      fields.set(key, toLil(value[key]));
    }
    const node = immObject(fields, keys);
    origins.set(node, value);
    return node;
  }
  const node = immNull();
  origins.set(node, null);
  return node;
}

function fromLil(node) {
  if (origins.has(node)) {
    return origins.get(node);
  }
  if (node.kind === 0) return null;
  if (node.kind === 1) return node.flag;
  if (node.kind === 2) return node.num;
  if (node.kind === 3) return node.text;
  if (node.kind === 4) {
    const out = [];
    for (let i = 0; i < node.items.length; i += 1) {
      out.push(fromLil(node.items[i]));
    }
    return out;
  }
  if (node.kind === 5) {
    const out = {};
    for (let i = 0; i < node.keys.length; i += 1) {
      const key = node.keys[i];
      const found = node.fields.get(key);
      if (found != null) out[key] = fromLil(found);
    }
    return out;
  }
  return null;
}

function materialize(value) {
  if (value == null) return undefined;
  if (isArrayDraft(value) || isObjectDraft(value) || value.isDraft) {
    return wrap(value);
  }
  return fromLil(value);
}

function wrap(draft) {
  const isArray = isArrayDraft(draft);
  const target = isArray ? [] : {};
  const proxy = new Proxy(target, {
    get(_t, prop) {
      if (prop === "constructor") return isArray ? Array : Object;
      if (isArray) {
        if (prop === "length") return draftLength(draft);
        if (prop === "push") {
          return (...args) => {
            let len = draftLength(draft);
            for (let i = 0; i < args.length; i += 1) {
              len = draftPush(draft, toLil(args[i]));
            }
            return len;
          };
        }
        if (prop === "pop") {
          return () => fromLil(draftPop(draft));
        }
        if (typeof prop === "string" && /^[0-9]+$/.test(prop)) {
          return materialize(draftGetIndex(draft, Number(prop)));
        }
      } else if (typeof prop === "string") {
        return materialize(draftGetProp(draft, prop));
      }
      return undefined;
    },
    set(_t, prop, value) {
      if (isArray && typeof prop === "string" && /^[0-9]+$/.test(prop)) {
        draftSetIndex(draft, Number(prop), toLil(value));
        return true;
      }
      if (typeof prop === "string") {
        draftSetProp(draft, prop, toLil(value));
        return true;
      }
      return false;
    },
    deleteProperty(_t, prop) {
      if (typeof prop === "string") {
        draftDeleteProp(draft, prop);
        return true;
      }
      return false;
    },
    ownKeys() {
      if (isArray) {
        const keys = [];
        const len = draftLength(draft);
        for (let i = 0; i < len; i += 1) keys.push(String(i));
        keys.push("length");
        return keys;
      }
      const snapshot = currentOf(draft);
      return snapshot.keys.slice();
    },
    getOwnPropertyDescriptor(_t, prop) {
      if (isArray && prop === "length") {
        return {
          configurable: true,
          enumerable: false,
          writable: true,
          value: draftLength(draft),
        };
      }
      const value = this.get(_t, prop);
      if (value === undefined) return undefined;
      return {
        configurable: true,
        enumerable: true,
        writable: true,
        value,
      };
    },
    has(_t, prop) {
      if (isArray && prop === "length") return true;
      return this.get(_t, prop) !== undefined;
    },
  });
  drafts.set(proxy, draft);
  return proxy;
}

function unwrap(value) {
  return drafts.get(value);
}

function pathFromLil(path) {
  const out = [];
  for (let i = 0; i < path.length; i += 1) {
    const elem = path[i];
    out.push(elem.isIndex ? elem.index : elem.key);
  }
  return out;
}

function patchesFromLil(patches) {
  const out = [];
  for (let i = 0; i < patches.length; i += 1) {
    const patch = patches[i];
    const item = { op: patch.op, path: pathFromLil(patch.path) };
    if (patch.hasValue) item.value = fromLil(patch.value);
    out.push(item);
  }
  return out;
}

function produce(base, recipe) {
  if (typeof base === "function" && recipe === undefined) {
    const curried = base;
    return (state) => produce(state, curried);
  }
  const rootValue = toLil(base);
  const rootDraft = createDraft(rootValue);
  const proxy = wrap(rootDraft);
  const result = recipe(proxy);
  if (result !== undefined && result !== proxy) {
    return result;
  }
  const finished = finishDraft(rootDraft);
  if (!draftModified(rootDraft) && finished === rootValue) {
    return base;
  }
  return fromLil(finished);
}

function produceWithPatches(base, recipe) {
  if (typeof base === "function" && recipe === undefined) {
    const curried = base;
    return (state) => produceWithPatches(state, curried);
  }
  if (!patchesEnabled) {
    throw new Error("enablePatches() must be called before produceWithPatches");
  }
  const rootValue = toLil(base);
  const rootDraft = createDraft(rootValue);
  const proxy = wrap(rootDraft);
  const result = recipe(proxy);
  if (result !== undefined && result !== proxy) {
    const replaced = replacementPatches(rootValue, toLil(result));
    return [result, patchesFromLil(replaced.patches), patchesFromLil(replaced.inversePatches)];
  }
  const finished = finishDraftWithPatches(rootDraft);
  if (!draftModified(rootDraft) && finished.value === rootValue) {
    return [base, [], []];
  }
  return [
    fromLil(finished.value),
    patchesFromLil(finished.patches),
    patchesFromLil(finished.inversePatches),
  ];
}

function deepClone(value) {
  if (value === null || typeof value !== "object") return value;
  if (Array.isArray(value)) {
    const out = [];
    for (let i = 0; i < value.length; i += 1) out.push(deepClone(value[i]));
    return out;
  }
  const out = {};
  const keys = Object.keys(value);
  for (let i = 0; i < keys.length; i += 1) {
    const key = keys[i];
    out[key] = deepClone(value[key]);
  }
  return out;
}

function applyPatches(base, patches) {
  if (!patchesEnabled) {
    throw new Error("enablePatches() must be called before applyPatches");
  }
  let root = deepClone(base);
  for (let p = 0; p < patches.length; p += 1) {
    const patch = patches[p];
    const path = patch.path;
    if (path.length === 0) {
      if (patch.op === "replace" || patch.op === "add") {
        root = deepClone(patch.value);
      }
      continue;
    }
    let target = root;
    for (let i = 0; i < path.length - 1; i += 1) {
      target = target[path[i]];
    }
    const key = path[path.length - 1];
    if (patch.op === "replace") {
      target[key] = deepClone(patch.value);
    } else if (patch.op === "add") {
      if (Array.isArray(target)) {
        if (key === "-") target.push(deepClone(patch.value));
        else target.splice(Number(key), 0, deepClone(patch.value));
      } else {
        target[key] = deepClone(patch.value);
      }
    } else if (patch.op === "remove") {
      if (Array.isArray(target)) target.splice(Number(key), 1);
      else delete target[key];
    }
  }
  return root;
}

function enablePatches() {
  patchesEnabled = true;
}

function current(value) {
  const draft = unwrap(value);
  if (draft == null) return value;
  return fromLil(currentOf(draft));
}

function original(value) {
  const draft = unwrap(value);
  if (draft == null) return undefined;
  return fromLil(originalOf(draft));
}

let passed = 0;
const parts = [];

function check(cond) {
  parts.push(cond ? "1" : "0");
  if (cond) passed += 1;
}

const base1 = { keep: { x: 1 }, edit: { y: 2 }, n: 5 };
const next1 = produce(base1, (d) => {
  d.edit.y = 9;
});
check(base1.edit.y === 2);
check(next1.edit.y === 9);
check(next1.keep === base1.keep);
check(next1.edit !== base1.edit);
check(next1.n === 5);

const base2 = { list: [1, 2], meta: { ok: true } };
const next2 = produce(base2, (d) => {
  d.list.push(3);
  d.list[0] = 8;
});
check(base2.list.length === 2);
check(next2.list.join(",") === "8,2,3");
check(next2.meta === base2.meta);

const base3 = { a: 1, b: 2, nested: { c: 3 } };
const next3 = produce(base3, (d) => {
  delete d.b;
  d.nested.c = 4;
});
check(!("b" in next3) && next3.a === 1 && next3.nested.c === 4);

const base4 = { a: 1 };
const next4 = produce(base4, () => {});
check(next4 === base4);

const base5 = { a: 1 };
const next5 = produce(base5, () => ({ b: 2 }));
check(next5.b === 2 && next5.a === undefined);

const inc = produce((d) => {
  d.n = d.n + 1;
});
check(inc({ n: 10 }).n === 11);

const base7 = { a: { b: 1 }, arr: [{ z: 1 }] };
produce(base7, (d) => {
  d.a.b = 9;
  d.arr[0].z = 7;
  d.arr.push({ z: 3 });
  check(original(d.a).b === 1);
  check(current(d.a).b === 9);
  check(original(d.arr[0]).z === 1);
  check(current(d.arr[0]).z === 7);
  check(current(d).arr.length === 2);
});

const base8 = { list: [1, 2, 3] };
const next8 = produce(base8, (d) => {
  d.list.pop();
});
check(next8.list.join(",") === "1,2");

enablePatches();

const base9 = { "keep": { "x": 1 }, "edit": { "y": 2 }, "list": [1, 2], "n": 5 };
const [next9, patches9, inverse9] = produceWithPatches(base9, (d) => {
  d["edit"]["y"] = 9;
  d["list"].push(3);
  d["list"][0] = 8;
  delete d["n"];
  d["added"] = { "z": 1 };
});
check(next9["edit"]["y"] === 9 && next9["list"].join(",") === "8,2,3" && next9["added"]["z"] === 1 && !("n" in next9));
check(patches9.length === 5);
check(patches9[0].op === "replace" && patches9[0].path.join("/") === "edit/y" && patches9[0].value === 9);
check(patches9[1].op === "replace" && patches9[1].path.join("/") === "list/0" && patches9[1].value === 8);
check(patches9[2].op === "add" && patches9[2].path.join("/") === "list/2" && patches9[2].value === 3);
check(patches9[3].op === "remove" && patches9[3].path.join("/") === "n");
check(patches9[4].op === "add" && patches9[4].path.join("/") === "added" && patches9[4].value["z"] === 1);
check(JSON.stringify(applyPatches(base9, patches9)) === JSON.stringify(next9));
check(JSON.stringify(applyPatches(next9, inverse9)) === JSON.stringify(base9));

const [next10, patches10, inverse10] = produceWithPatches({ "a": 1 }, () => {});
check(next10["a"] === 1 && patches10.length === 0 && inverse10.length === 0);

const [next11, patches11] = produceWithPatches({ "a": 1 }, () => ({ "b": 2 }));
check(next11["b"] === 2 && patches11.length === 1 && patches11[0].op === "replace" && patches11[0].path.length === 0);

const [next12, patches12, inverse12] = produceWithPatches({ "list": [1, 2, 3] }, (d) => {
  d["list"].pop();
});
check(next12["list"].join(",") === "1,2");
check(patches12.length === 1 && patches12[0].op === "remove" && patches12[0].path.join("/") === "list/2");
check(applyPatches({ "list": [1, 2, 3] }, patches12)["list"].join(",") === "1,2");
check(applyPatches(next12, inverse12)["list"].join(",") === "1,2,3");

console.log(`immer:${passed}:${parts.join("")}`);
