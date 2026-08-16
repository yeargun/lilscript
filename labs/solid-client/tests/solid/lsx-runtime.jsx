import {
  createResource,
  createSignal,
  ErrorBoundary,
  onCleanup,
  Suspense,
} from "solid-js";
import {
  Dynamic,
  For,
  Index,
  Match,
  Portal,
  Show,
  Switch,
  render,
} from "solid-js/web";

globalThis.registerLsxDispose ??= (dispose) => {
  globalThis.__disposeLsx = dispose;
};

const [count, setCount] = createSignal(0);
const [visible, setVisible] = createSignal(true);
const [rows, setRows] = createSignal([1, 2]);
const [indexed, setIndexed] = createSignal([4, 5]);
const [selected, setSelected] = createSignal(0);
const [dynamicTag, setDynamicTag] = createSignal("aside");
const [componentCleanups, setComponentCleanups] = createSignal(0);
const [dynamicCleanups, setDynamicCleanups] = createSignal(0);
const [rowCleanups, setRowCleanups] = createSignal(0);
const [forFallbackCleanups, setForFallbackCleanups] = createSignal(0);
const [boundaryFault, setBoundaryFault] = createSignal(false);
const [boundaryCleanups, setBoundaryCleanups] = createSignal(0);
const [initialBoundaryCleanups, setInitialBoundaryCleanups] = createSignal(0);
const [suspenseContentCleanups, setSuspenseContentCleanups] = createSignal(0);
const [suspenseFallbackCleanups, setSuspenseFallbackCleanups] = createSignal(0);
const [indexedCleanups, setIndexedCleanups] = createSignal(0);
const [nestedCleanups, setNestedCleanups] = createSignal(0);
const [keyedShowCleanups, setKeyedShowCleanups] = createSignal(0);
const [keyedMatchCleanups, setKeyedMatchCleanups] = createSignal(0);
const [dynamicMode, setDynamicMode] = createSignal(0);
const [dynamicChoice, setDynamicChoice] = createSignal("aside");
const [dynamicSvgTag, setDynamicSvgTag] = createSignal("path");
const [spreadClicks, setSpreadClicks] = createSignal(0);
const [spreadTitle, setSpreadTitle] = createSignal("spread-initial");
const [spreadEnabled, setSpreadEnabled] = createSignal(true);
const [spreadHandler, setSpreadHandler] = createSignal(() => {});
const greetingSpread = { tone: "spread" };
let spreadButton;
let liveMatchAccessor = () => -1;
let resolveSuspenseFirst = () => {};
let resolveSuspenseSecond = () => {};

function Greeting(props) {
  onCleanup(() => setComponentCleanups((value) => value + 1));
  return (
    <p data-component={props.tone}>
      {props.label}
      {props.children}
    </p>
  );
}

function DynamicGreeting(props) {
  onCleanup(() => setDynamicCleanups((value) => value + 1));
  return (
    <p data-dynamic-component="greeting">
      {props.label}
      {props.children}
    </p>
  );
}

function RowView(props) {
  onCleanup(() => setRowCleanups((value) => value + 1));
  return (
    <li data-row={props.row}>
      {props.row}:{props.position()}
    </li>
  );
}

function ForFallback() {
  onCleanup(() => setForFallbackCleanups((value) => value + 1));
  return <li data-row="empty">Empty</li>;
}

function boundaryLabel() {
  if (boundaryFault()) throw new Error("Boundary failure");
  return "Healthy";
}

function BoundaryOwned() {
  onCleanup(() => setBoundaryCleanups((value) => value + 1));
  return <p data-boundary="healthy">{boundaryLabel()}</p>;
}

function InitialBoundaryFailure() {
  onCleanup(() => setInitialBoundaryCleanups((value) => value + 1));
  throw new Error("Initial boundary failure");
}

function SuspenseOwned() {
  const [firstResource] = createResource(
    () =>
      new Promise((resolve) => {
        resolveSuspenseFirst = resolve;
      }),
  );
  const [secondResource] = createResource(
    () =>
      new Promise((resolve) => {
        resolveSuspenseSecond = resolve;
      }),
  );
  onCleanup(() => setSuspenseContentCleanups((value) => value + 1));
  return (
    <p data-suspense="content">
      {firstResource()} + {secondResource()}
    </p>
  );
}

function SuspenseFallback() {
  onCleanup(() => setSuspenseFallbackCleanups((value) => value + 1));
  return <p data-suspense="fallback">Loading</p>;
}

function IndexedRow(props) {
  onCleanup(() => setIndexedCleanups((value) => value + 1));
  return <li data-index={props.position}>{props.value()}</li>;
}

function NestedOwned(props) {
  onCleanup(() => setNestedCleanups((value) => value + 1));
  return <mark data-nested="owned">{props.label}</mark>;
}

function TopLevelControl() {
  return (
    <Show when={visible()} fallback={<u data-top-level="hidden">Top hidden</u>}>
      <u data-top-level="shown">Top shown</u>
      <span data-top-level-tail="shown">Tail</span>
    </Show>
  );
}

function KeyedShowOwned(props) {
  onCleanup(() => setKeyedShowCleanups((value) => value + 1));
  return <output data-keyed-show={props.value}>{props.value}</output>;
}

function KeyedMatchOwned(props) {
  onCleanup(() => setKeyedMatchCleanups((value) => value + 1));
  return <output data-keyed-match={props.value}>{props.value}</output>;
}

function LiveMatchOwned(props) {
  liveMatchAccessor = props.value;
  return <output data-live-match="active">{props.value()}</output>;
}

function checkStaleMatch() {
  let status = "live";
  try {
    liveMatchAccessor();
  } catch {
    status = "throw";
  }
  spreadButton.setAttribute("data-stale-match", status);
}

function spreadHit(amount) {
  const next = spreadClicks() + amount;
  setSpreadClicks(next);
  spreadButton.setAttribute("data-handler-count", String(next));
}

function updateSpread() {
  setSpreadTitle("spread-updated");
  setSpreadEnabled(false);
  setSpreadHandler(() => () => spreadHit(10));
}

setSpreadHandler(() => () => spreadHit(1));
const hostSpread = {
  get "data-order"() {
    return "spread";
  },
  get "data-action"() {
    return "spread";
  },
  get title() {
    return spreadTitle();
  },
  get ref() {
    return (node) => {
      spreadButton = node;
      node.setAttribute("data-spread-ref", "captured");
    };
  },
  get onClick() {
    return spreadHandler();
  },
  get classList() {
    return {
      ready: spreadEnabled(),
      "spread pair": spreadEnabled(),
    };
  },
  get style() {
    return {
      "--spread-count": String(spreadClicks()),
      color: spreadEnabled() ? "green" : "purple",
    };
  },
};
const inputSpread = {
  get value() {
    return String(spreadClicks());
  },
  get checked() {
    return spreadEnabled();
  },
};

function captureRef(node) {
  node.setAttribute("data-ref", "captured");
}

function captureShadowPortal(node) {
  node.setAttribute("data-shadow-ref", "captured");
}

// JSX directive names are compile-time references that ESLint cannot see.
// eslint-disable-next-line no-unused-vars
function mark(node, value) {
  node.setAttribute(
    "data-directive",
    typeof value === "function" ? value() : value,
  );
}

const dispose = render(
  () => (
    <section
      id="lsx-root"
      data-count={count()}
      classList={{ active: visible(), counted: count() > 0 }}
      data-component-cleanups={componentCleanups()}
      data-dynamic-cleanups={dynamicCleanups()}
      data-row-cleanups={rowCleanups()}
      data-for-fallback-cleanups={forFallbackCleanups()}
      data-boundary-cleanups={boundaryCleanups()}
      data-initial-boundary-cleanups={initialBoundaryCleanups()}
      data-suspense-content-cleanups={suspenseContentCleanups()}
      data-suspense-fallback-cleanups={suspenseFallbackCleanups()}
      data-indexed-cleanups={indexedCleanups()}
      data-nested-cleanups={nestedCleanups()}
      data-keyed-show-cleanups={keyedShowCleanups()}
      data-keyed-match-cleanups={keyedMatchCleanups()}
      data-spread-clicks={spreadClicks()}
    >
      <h1>Count {count()}</h1>
      <input
        required
        value={count()}
        checked={visible()}
        disabled={!visible()}
      />
      <button
        type="button"
        data-action="increment"
        onClick={() => setCount(count() + 1)}
      >
        Increment
      </button>
      <button
        type="button"
        data-action="toggle"
        onClick={() => setVisible(!visible())}
      >
        Toggle
      </button>
      <button
        type="button"
        data-action="rows"
        onClick={() => setRows([2, 1, 3])}
      >
        Rows
      </button>
      <button
        type="button"
        data-action="rows-duplicate"
        onClick={() => setRows([1, 1, 3])}
      >
        Duplicate rows
      </button>
      <button
        type="button"
        data-action="rows-remove"
        onClick={() => setRows([1, 3])}
      >
        Remove row
      </button>
      <button
        type="button"
        data-action="rows-clear"
        onClick={() => setRows([])}
      >
        Clear rows
      </button>
      <button
        type="button"
        data-action="rows-restore"
        onClick={() => setRows([3, 4])}
      >
        Restore rows
      </button>
      <button
        type="button"
        data-action="indexed"
        onClick={() => setIndexed([7])}
      >
        Indexed
      </button>
      <button
        type="button"
        data-action="boundary-fail"
        onClick={() => setBoundaryFault(true)}
      >
        Fail boundary
      </button>
      <button
        type="button"
        data-action="suspense-resolve-first"
        onClick={() => resolveSuspenseFirst("First")}
      >
        Resolve first suspense resource
      </button>
      <button
        type="button"
        data-action="suspense-resolve-second"
        onClick={() => resolveSuspenseSecond("Second")}
      >
        Resolve second suspense resource
      </button>
      <button
        type="button"
        data-action="switch"
        onClick={() => setSelected((value) => (value + 1) % 3)}
      >
        Switch
      </button>
      <button type="button" data-action="stale-match" onClick={checkStaleMatch}>
        Check stale match
      </button>
      <button
        type="button"
        data-action="dynamic"
        onClick={() => {
          if (dynamicMode() === 0) {
            setDynamicChoice(() => DynamicGreeting);
            setDynamicSvgTag("circle");
            setDynamicMode(1);
          } else if (dynamicMode() === 1) {
            setDynamicChoice("article");
            setDynamicTag("article");
            setDynamicSvgTag("path");
            setDynamicMode(2);
          } else if (dynamicMode() === 2) {
            setDynamicChoice(null);
            setDynamicMode(3);
          } else {
            setDynamicChoice("aside");
            setDynamicTag("aside");
            setDynamicMode(0);
          }
        }}
      >
        Dynamic
      </button>
      <button type="button" data-action="spread-update" onClick={updateSpread}>
        Update spread
      </button>
      <div data-action="native" onScroll={() => setCount(count() + 10)}>
        Native event
      </div>
      <p ref={captureRef} use:mark={"ready"}>
        Owned directives
      </p>
      <div data-control="show">
        <Show when={visible()} fallback={<em data-state="hidden">Hidden</em>}>
          <strong data-state="shown">Shown</strong>
        </Show>
      </div>
      <ul data-control="for">
        <For each={rows()} fallback={<ForFallback />}>
          {(row, index) => <RowView row={row} position={index} />}
        </For>
      </ul>
      <ol data-control="index">
        <Index each={indexed()} fallback={<li data-index="empty">Empty</li>}>
          {(value, index) => <IndexedRow value={value} position={index} />}
        </Index>
      </ol>
      <div data-control="boundary">
        <ErrorBoundary
          fallback={(error, reset) => (
            <button
              type="button"
              data-action="boundary-reset"
              onClick={() => {
                setBoundaryFault(false);
                reset();
              }}
            >
              {error.message}
            </button>
          )}
        >
          <BoundaryOwned />
        </ErrorBoundary>
        <ErrorBoundary
          fallback={
            <span data-boundary-initial="fallback">
              Initial boundary failure
            </span>
          }
        >
          <InitialBoundaryFailure />
        </ErrorBoundary>
      </div>
      <div data-control="suspense">
        <Suspense fallback={<SuspenseFallback />}>
          <SuspenseOwned />
        </Suspense>
      </div>
      <div data-control="switch">
        <Switch fallback={<i data-switch="fallback">Fallback</i>}>
          <Match when={selected() === 1}>
            <Greeting
              {...greetingSpread}
              tone="after"
              label={`First ${count()}`}
            >
              <span data-component-child="present"> Child</span>
            </Greeting>
          </Match>
          <Match when={selected() === 2}>
            <b data-switch="second">Second</b>
          </Match>
        </Switch>
      </div>
      <div data-control="dynamic">
        <Dynamic
          component={dynamicChoice()}
          data-dynamic={dynamicTag()}
          label={`Dynamic ${count()}`}
        >
          <small data-dynamic-child="present"> Child</small>
        </Dynamic>
      </div>
      <div data-control="nested">
        <Show
          when={visible()}
          fallback={<small data-nested="outer-fallback">Outer fallback</small>}
        >
          <span data-nested="prefix">Prefix</span>
          <Switch
            fallback={<i data-nested="switch-fallback">Nested fallback</i>}
          >
            <Match when={selected() === 2}>
              <NestedOwned label={`Nested ${count()}`} />
            </Match>
          </Switch>
        </Show>
      </div>
      <TopLevelControl />
      <div data-control="keyed">
        <Show
          when={count()}
          keyed
          fallback={<i data-keyed-show="fallback">Show fallback</i>}
        >
          {(value) => <KeyedShowOwned value={value} />}
        </Show>
        <Show
          when={count()}
          fallback={<i data-live-show="fallback">Live fallback</i>}
        >
          {(value) => <output data-live-show={value()}>{value()}</output>}
        </Show>
        <Switch fallback={<i data-keyed-match="fallback">Match fallback</i>}>
          <Match when={selected() === 2}>
            <span data-keyed-match="priority">Priority</span>
          </Match>
          <Match when={count()} keyed>
            {(value) => <KeyedMatchOwned value={value} />}
          </Match>
        </Switch>
        <Switch
          fallback={<i data-live-match="fallback">Live match fallback</i>}
        >
          <Match when={selected() !== 2 && count()}>
            {(value) => <LiveMatchOwned value={value} />}
          </Match>
        </Switch>
      </div>
      <div data-control="spread">
        <button {...hostSpread} data-order="after">
          Spread
        </button>
        <input {...inputSpread} data-spread-input="present" />
      </div>
      <Portal mount={document.querySelector("#portal-target")}>
        <button
          type="button"
          data-action="portal"
          onClick={() => setCount(count() + 100)}
        >
          Portal {count()}
        </button>
      </Portal>
      <Portal mount={document.head}>
        <title>SolidLil {count()}</title>
      </Portal>
      <Portal mount={document.querySelector("#svg-portal-target")} isSVG>
        <circle data-portal-svg="circle" cx="3" cy="3" r="2" />
      </Portal>
      <Portal
        mount={document.querySelector("#shadow-portal-target")}
        useShadow
        ref={captureShadowPortal}
      >
        <span data-portal-shadow="content">Shadow {count()}</span>
      </Portal>
      <svg data-namespace="svg" viewBox="0 0 10 10">
        <circle data-shape="circle" cx="5" cy="5" r="4" />
        <use data-shape="use" xlink:href={visible() ? "#shown" : "#hidden"} />
        <text data-xml="language" xml:lang={visible() ? "en" : "tr"}>
          Language
        </text>
        <Dynamic
          component={dynamicSvgTag()}
          data-dynamic-svg={dynamicSvgTag()}
        />
      </svg>
      <math data-namespace="math">
        <mi>x</mi>
      </math>
    </section>
  ),
  document.querySelector("#app"),
);

globalThis.registerLsxDispose(dispose);
globalThis.registerLsxBoundaryDiagnostics?.(
  () => boundaryCleanups(),
  () => initialBoundaryCleanups(),
  () => suspenseContentCleanups(),
  () => suspenseFallbackCleanups(),
);
