import { configureStore, createSlice } from "@reduxjs/toolkit";

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
