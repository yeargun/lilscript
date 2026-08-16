import { batch, createEffect, createMemo, createSignal } from "solid-js";
import { render } from "solid-js/web";
import "../../shared/app.css";

function App() {
  const [count, setCount] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  const even = createMemo(() => count() % 2 === 0);

  createEffect(() => {
    document.documentElement.dataset.count = String(count());
  });

  const burst = () => {
    batch(() => {
      for (let index = 0; index < 100; index += 1)
        setCount((value) => value + 1);
    });
  };

  return (
    <section class="shell" data-runtime="solid">
      <header>
        <p>Reactive runtime comparison</p>
        <h1>SolidJS baseline</h1>
      </header>
      <div class="values" aria-live="polite">
        <div>
          <span>Count</span>
          <strong data-value="count">{count()}</strong>
        </div>
        <div>
          <span>Doubled</span>
          <strong data-value="doubled">{doubled()}</strong>
        </div>
        <div>
          <span>Parity</span>
          <strong data-value="parity">{even() ? "Even" : "Odd"}</strong>
        </div>
      </div>
      <div class="actions">
        <button
          type="button"
          data-action="increment"
          onClick={() => setCount((value) => value + 1)}
        >
          Increment
        </button>
        <button type="button" data-action="burst" onClick={burst}>
          Add 100
        </button>
        <button type="button" data-action="reset" onClick={() => setCount(0)}>
          Reset
        </button>
      </div>
    </section>
  );
}

globalThis.__disposeSolidBenchmark = render(
  () => <App />,
  document.querySelector("#app"),
);
