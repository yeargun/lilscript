import {
  computed,
  effect,
  isReactive,
  isRef,
  reactive,
  ref,
  stop,
} from "vue";

const state = reactive({ count: 1, items: [2, 4] });
const multiplier = ref(3);
const total = computed(
  () => state.count * multiplier.value + state.items.reduce((sum, value) => sum + value, 0),
);
const snapshots = [];
const runner = effect(() => snapshots.push(total.value));

state.count = 2;
state.items.push(5);
multiplier.value = 4;
state.items[0] = 7;
stop(runner);
state.count = 9;

globalThis.__VUELIL_REACTIVITY_RESULT__ = JSON.stringify({
  snapshots,
  finalTotal: total.value,
  reactive: isReactive(state),
  ref: isRef(multiplier),
});
