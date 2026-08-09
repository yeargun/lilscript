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
  makeAction,
  makeActionType,
  getState as lilGetState,
  dispatch as lilDispatch,
  combineReduce,
  configureStoreState,
} from "../../../build/redux-toolkit-lilscript.js";

const drafts = new WeakMap();
const origins = new WeakMap();

function toLil(value) {
  if (value === null || value === undefined) {
    const node = immNull();
    origins.set(node, value === undefined ? null : value);
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

function createSlice({ name, initialState, reducers }) {
  const actions = {};
  const caseReducers = {};
  const reducerNames = Object.keys(reducers);
  for (let i = 0; i < reducerNames.length; i += 1) {
    const key = reducerNames[i];
    const type = makeActionType(name, key);
    const actionCreator = (payload) => ({ type, payload });
    actionCreator.type = type;
    actions[key] = actionCreator;
    caseReducers[type] = reducers[key];
  }

  const initialLil = toLil(initialState);

  function reducer(stateLil, action) {
    const current = stateLil == null ? initialLil : stateLil;
    const caseReducer = caseReducers[action.type];
    if (!caseReducer) return current;
    const stateJs = fromLil(current);
    const payload = action.hasPayload ? fromLil(action.payload) : undefined;
    const nextJs = produce(stateJs, (draft) =>
      caseReducer(draft, { type: action.type, payload }),
    );
    if (nextJs === stateJs) return current;
    return toLil(nextJs);
  }

  reducer.initialLil = initialLil;
  reducer.getInitialState = () => initialState;

  return {
    name,
    reducer,
    actions,
    caseReducers: reducers,
    getInitialState: () => initialState,
  };
}

function configureStore({ reducer: reducerMap }) {
  const keys = Object.keys(reducerMap);
  const initials = [];
  const reduces = [];
  for (let i = 0; i < keys.length; i += 1) {
    const key = keys[i];
    const sliceReducer = reducerMap[key];
    initials.push(
      sliceReducer.initialLil ??
        toLil(
          typeof sliceReducer.getInitialState === "function"
            ? sliceReducer.getInitialState()
            : {},
        ),
    );
    reduces.push(sliceReducer);
  }

  const store = configureStoreState(keys, initials);
  const rootReducer = (state, action) =>
    combineReduce(state, action, keys, initials, reduces);

  return {
    getState() {
      return fromLil(lilGetState(store));
    },
    dispatch(action) {
      const lilAction = makeAction(
        action.type,
        action.payload === undefined ? immNull() : toLil(action.payload),
        action.payload !== undefined,
      );
      lilDispatch(store, lilAction, rootReducer);
      return action;
    },
  };
}

let passed = 0;
const parts = [];

function check(cond) {
  parts.push(cond ? "1" : "0");
  if (cond) passed += 1;
}

const counterSlice = createSlice({
  name: "counter",
  initialState: { value: 0 },
  reducers: {
    increment(state) {
      state.value += 1;
    },
    decrement(state) {
      state.value -= 1;
    },
  },
});

const todosSlice = createSlice({
  name: "todos",
  initialState: { items: [] },
  reducers: {
    addTodo(state, action) {
      state.items.push({
        id: action.payload.id,
        text: action.payload.text,
        done: false,
      });
    },
    removeTodo(state, action) {
      const id = action.payload;
      const items = [];
      for (let i = 0; i < state.items.length; i += 1) {
        const item = state.items[i];
        if (item.id !== id) {
          items.push({ id: item.id, text: item.text, done: item.done });
        }
      }
      return { items };
    },
    toggleTodo(state, action) {
      const id = action.payload;
      for (let i = 0; i < state.items.length; i += 1) {
        if (state.items[i].id === id) {
          state.items[i].done = !state.items[i].done;
          break;
        }
      }
    },
  },
});

const { increment, decrement } = counterSlice.actions;
const { addTodo, removeTodo, toggleTodo } = todosSlice.actions;

const store = configureStore({
  reducer: {
    counter: counterSlice.reducer,
    todos: todosSlice.reducer,
  },
});

store.dispatch(increment());
store.dispatch(increment());
store.dispatch(decrement());
check(store.getState().counter.value === 1);

store.dispatch(addTodo({ id: 1, text: "x" }));
store.dispatch(addTodo({ id: 2, text: "y" }));
store.dispatch(toggleTodo(1));
store.dispatch(removeTodo(2));

const todos = store.getState().todos.items;
check(todos.length === 1);
check(todos[0].id === 1 && todos[0].text === "x" && todos[0].done === true);
check(increment().type === "counter/increment");
check(addTodo({ id: 0, text: "" }).type === "todos/addTodo");

console.log(
  `rtk:${passed}:${parts.join("")}:${store.getState().counter.value}:${todos.length}`,
);
