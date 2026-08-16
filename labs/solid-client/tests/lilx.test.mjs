import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { compileLilx } from "../tooling/lilx/compile.mjs";
import { createDirectDomWebSource } from "../tooling/lilx/direct-dom.mjs";
import { createEffectOnlyReactiveSource } from "../tooling/lilx/direct-reactive.mjs";
import { parseJsx } from "../tooling/lilx/parse-jsx.mjs";

test("lowers a declared For fallback without app-specific output", () => {
  const source = `
    import { For, Signal } from "solidlil";
    Signal<Row[]> rows;
    DomNode view() {
      return (
        <ul className="rows">
          <For each={rows} fallback={<li class="vacant">No rows</li>}>
            {(Row row, Signal<int> index) => (<li>{row.label}</li>)}
          </For>
        </ul>
      );
    }
  `;
  const output = compileLilx(source, { filename: "fallback.lilx" });
  assert.match(output, /prepareTemplate\(/);
  assert.match(output, /cloneTemplate\(/);
  assert.match(output, /vacant/);
  assert.match(output, /No rows/);
  assert.doesNotMatch(output, /element\("li"\)/);
  assert.doesNotMatch(output, /Nothing here/);
});

test("uses the allocation-free value mapper for a single For row", () => {
  const source = `
    import { For, Signal } from "solidlil";
    Signal<Row[]> rows;
    DomNode view() {
      return <ul><For each={rows}>{(Row row) => (<li>{row.label}</li>)}</For></ul>;
    }
  `;
  const output = compileLilx(source, { filename: "empty.lilx" });
  assert.match(output, /dynamicForValue\(/);
  assert.doesNotMatch(output, /dynamicForValueNodes\(/);
  assert.doesNotMatch(output, /fallbackNodes/);
});

test("reports mismatched JSX tags with the source filename", () => {
  assert.throws(
    () =>
      compileLilx("DomNode view() { return <div></span>; }", {
        filename: "broken.lilx",
      }),
    /broken\.lilx: mismatched <\/span>/,
  );
});

test("isolates dynamic property operations in their own render effects", () => {
  const output = compileLilx(
    "DomNode view() { return <div classList={{ ready: ready.read(), counted: count.read() > 0 }} data-count={count.read()} />; }",
    { filename: "grouped.lilx" },
  );
  assert.equal(output.match(/createRenderEffect\(/g)?.length, 3);
  assert.match(output, /classToggle\(/);
  assert.match(output, /attribute\(/);
});

test("preserves nested braces and strings inside expressions", () => {
  const { node } = parseJsx(
    '<button title={format({ label: "}" })} onClick={() => save({ id: 1 })}>Go</button>',
  );
  assert.equal(node.props[0].value, 'format({ label: "}" })');
  assert.equal(node.props[1].value, "() => save({ id: 1 })");
});

test("preserves the JSX separator before a multiline reactive expression", () => {
  const output = compileLilx(`
    DomNode view() {
      return (
        <button>
          Count {count.read()}
        </button>
      );
    }
  `);
  assert.match(output, /Count /);
  assert.match(output, /dynamicTextNode\(/);
  assert.doesNotMatch(output, /text\("Count "\)/);
});

test("does not allocate an effect for a non-reactive typed field", () => {
  const output = compileLilx(
    "DomNode view() { return <p>{row.id} {row.label.read()}</p>; }",
  );
  assert.match(output, /setText\([^,]+, "" \+ \(row\.id\)\)/);
  assert.match(output, /dynamicTextNode\([^,]+, \(\) => "" \+ \(row\.label\.read\(\)\)\)/);
  assert.doesNotMatch(output, /createRenderEffect\(\(\) => \{ setText\(/);
  assert.doesNotMatch(output, /append\([^,]+, text\("" \+ \(row\.id\)\)\)/);
});

test("hoists HTML templates to module scope and interns identical trees", () => {
  const output = compileLilx(`
    DomNode first() { return <button type="button">Go</button>; }
    DomNode second() { return <button type="button">Go</button>; }
  `);
  assert.equal(output.match(/prepareTemplate\(/g)?.length, 1);
  assert.equal(output.match(/cloneTemplate\(/g)?.length, 2);
  assert.match(
    output,
    /JsValue _tmpl\d+ = prepareTemplate\("[^"]+"\);[\s\S]*cloneTemplate\(_tmpl\d+\)/,
  );
  assert.doesNotMatch(output, /element\("button"\)/);
});

test("emits a bare clone for a static host tree with no holes", () => {
  const output = compileLilx(
    "DomNode view() { return <p class=\"vacant\">No rows</p>; }",
  );
  assert.match(output, /return cloneTemplate\(_tmpl\d+\);/);
  assert.doesNotMatch(
    output,
    /return \( \(\) => \{\n  DomNode _el\d+ = cloneTemplate\(_tmpl\d+\);\n  return _el\d+;\n\}\)\(\);/,
  );
});

test("keeps host spreads on the createElement path", () => {
  const output = compileLilx(
    'DomNode view() { return <div before="yes" {...props} after={name.read()} />; }',
  );
  assert.match(output, /element\("div"\)/);
  assert.match(output, /spreadProps\(/);
});

test("supports monorepo-relative runtime and DOM imports", () => {
  const output = compileLilx("DomNode view() { return <main>Hi</main>; }", {
    reactiveImport: "../../lilscript/src/reactive",
    domImport: "./dom",
  });
  assert.match(output, /from "\.\.\/\.\.\/lilscript\/src\/reactive"/);
  assert.match(output, /from "\.\/dom"/);
  assert.match(output, /prepareTemplate\(/);
  assert.match(output, /cloneTemplate\(/);
});

test("emits a typed lexical host seam for direct-DOM builds", () => {
  const output = compileLilx("DomNode view() { return <main>Hi</main>; }", {
    hostImport: "./direct-dom-host.js",
  });
  assert.match(
    output,
    /import extern \{[^}]*domCreateElement[^}]*hostSchedule[^}]*\} from "\.\/direct-dom-host\.js";/,
  );
});

test("erases closed-world DomNode wrappers into transparent host handles", () => {
  const web = createDirectDomWebSource(`
export class DomNode {
  JsValue id;
  init(JsValue id) { this.id = id; }
}
export DomNode element() { return new DomNode(domCreateElement("p")); }
export void append(DomNode parent, DomNode child) {
  domAppendChild(parent.id, child.id);
}
export class DomEvent {
  JsValue id;
  init(JsValue id) { this.id = id; }
  DomNode target() { return new DomNode(domEventTarget(this.id)); }
}
  `);
  assert.doesNotMatch(web, /class DomNode/);
  assert.match(web, /export JsValue element\(\) \{ return domCreateElement\("p"\); \}/);
  assert.match(web, /domAppendChild\(parent, child\)/);
  assert.match(web, /JsValue target\(\) \{ return domEventTarget\(this\.id\); \}/);

  const coreWeb = createDirectDomWebSource(
    `
enableSuspenseResolution(() => useContext(suspenseContext));
export int createRenderEffect(func()->void callback) {
  return createReactiveRenderEffect(() => {
    try { callback(); } catch (auto error) { routeDomError(error, getOwner()); }
  });
}
export class DomNode { JsValue id; init(JsValue id) { this.id = id; } }
export class DomEvent { JsValue id; init(JsValue id) { this.id = id; } }
`,
    { errorBoundary: false, suspense: false },
  );
  assert.match(coreWeb, /return createReactiveRenderEffect\(callback\)/);
  assert.doesNotMatch(coreWeb, /enableSuspenseResolution/);

  const closedWeb = createDirectDomWebSource(
    readFileSync(
      resolve(import.meta.dirname, "../apps/lilscript/src/web.lil"),
      "utf8",
    ),
    { errorBoundary: false, suspense: false },
  );
  assert.match(
    closedWeb,
    /domReconcile\(this\.parent, this\.marker, this\.current, next\);/,
  );
  assert.doesNotMatch(closedWeb, /currentIds/);

  const output = compileLilx("DomNode view() { return <p />; }", {
    domImport: "./web-direct",
    directDom: true,
  });
  assert.doesNotMatch(output, /\bDomNode\b/);
  assert.match(output, /JsValue view\(\)/);
  assert.match(output, /cloneTemplate\(/);
  assert.doesNotMatch(output, /flattenNodeGroups\(\[nodeGroup\(/);
});

test("specializes a memo-free reactive graph to FIFO effects", () => {
  const source = `
class Signal<T> {
  T read() { return this.value; }
  T write(T next) {
    if (activeEffectId >= 0 && effects[activeEffectId].memoComputation) {
      this.level = effects[activeEffectId].level;
      this.producerEffectId = activeEffectId;
    }
    return next;
  }
}
void queueEffect(int effectId) { complexQueue(); }
int takeNextEffect() { return complexSelection(); }
void flushEffects() { throw "Potential Infinite Loop Detected."; }
void flushPureEffects() { complexPureFlush(); }
void scheduleObservers(int[] observerIds) { complexSchedule(); }
`;
  const output = createEffectOnlyReactiveSource(source);
  assert.match(output, /return takeQueuedEffectAt\(0\)/);
  assert.match(output, /effects\[effectId\]\.sources\.push/);
  assert.doesNotMatch(output, /complexQueue|complexSelection|complexPureFlush|complexSchedule/);
  assert.doesNotMatch(output, /Potential Infinite Loop Detected/);
});

test("lowers user components with ordered live props and component spreads", () => {
  const output = compileLilx(`
    DomNode view() {
      return <main><Greeting before="first" {...shared} after={name.read()} /></main>;
    }
  `);
  assert.match(output, /JsValue _props\d+ = componentProps\(\)/);
  assert.match(output, /componentProperty\([^,]+, "before", \(\) => "first"\)/);
  assert.match(output, /componentSpread\([^,]+, shared\)/);
  assert.match(
    output,
    /componentProperty\([^,]+, "after", \(\) => name\.read\(\)\)/,
  );
  assert.match(output, /componentNode\(Greeting, _props\d+\)/);
});

test("lowers host spreads and later explicit props in source order", () => {
  const output = compileLilx(
    'DomNode view() { return <div before="yes" {...props} after={name.read()} />; }',
  );
  assert.match(output, /componentProperty\([^,]+, "before", \(\) => "yes"\)/);
  assert.match(output, /componentSpread\([^,]+, props\)/);
  assert.match(
    output,
    /componentProperty\([^,]+, "after", \(\) => name\.read\(\)\)/,
  );
  assert.match(output, /spreadProps\([^,]+, [^,]+, false\)/);
});

test("wraps a top-level control-flow component in a movable fragment", () => {
  const output = compileLilx(`
    DomNode view() {
      return <Show when={ready.read()}><span>Ready</span></Show>;
    }
  `);
  assert.match(output, /DomNode _fragment\d+ = fragment\(\)/);
  assert.match(output, /dynamicShowValue\(region\(_fragment\d+\)/);
  assert.match(output, /materializeNodeGroup\(_nodes\d+\)/);
});

test("composes nested control flow and component children as node groups", () => {
  const output = compileLilx(`
    DomNode view() {
      return (
        <Show when={ready.read()}>
          <Greeting />
          <Switch><Match when={first.read()}><b>First</b></Match></Switch>
        </Show>
      );
    }
  `);
  assert.match(output, /componentNodes\(Greeting/);
  assert.match(output, /dynamicSwitchValues\(/);
  assert.match(output, /childNodes\(_fragment\d+\)/);
  assert.match(output, /flattenNodeGroups\(/);
});

test("lowers component rows and fallbacks through keyed node arrays", () => {
  const output = compileLilx(`
    DomNode view() {
      return (
        <For each={rows} fallback={<Empty />}>
          {(Row row) => <RowView row={row} />}
        </For>
      );
    }
  `);
  assert.match(output, /dynamicForValueNodes\(/);
  assert.match(output, /componentNodes\(RowView/);
  assert.match(output, /componentNodes\(Empty/);
  assert.match(output, /DomNode\[\] nodes = flattenNodeGroups\(/);
  assert.match(output, /DomNode\[\] fallbackNodes = flattenNodeGroups\(/);
});

test("accepts full LilScript types in control-flow callbacks", () => {
  const output = compileLilx(`
    DomNode view() {
      return (
        <main>
          <For each={rows}>
            {(Box<(int | string)[]>? row, Signal<int> position) => (
              <RowView row={row} position={position} />
            )}
          </For>
          <Index each={indexed}>
            {(Signal<Box<(int | string)[]>?> row, int position) => (
              <RowView row={row} position={position} />
            )}
          </Index>
          <Show when={selected.read()} keyed>
            {(func(int, string)->Box<int[]> value) => <Result value={value} />}
          </Show>
        </main>
      );
    }
  `);
  assert.match(
    output,
    /\(Box<\(int \| string\)\[\]>\? row, Signal<int> position\) =>/,
  );
  assert.match(
    output,
    /\(Signal<Box<\(int\|string\)\[\]>\?> row, int position\) =>/,
  );
  assert.match(output, /func\(int, string\)->Box<int\[\]> value/);
});

test("rejects invalid For and Index callback contracts", () => {
  assert.throws(
    () =>
      compileLilx(
        "DomNode view() { return <For each={rows}>{(int row, int index) => <p />}</For>; }",
      ),
    /For index parameter must have type Signal<int>/,
  );
  assert.throws(
    () =>
      compileLilx(
        "DomNode view() { return <Index each={rows}>{(int row) => <p />}</Index>; }",
      ),
    /Index value parameter must have type Signal<T>/,
  );
});

test("lowers typed ErrorBoundary fallbacks with reset support", () => {
  const output = compileLilx(`
    DomNode view() {
      return (
        <main>
          <ErrorBoundary
            fallback={(JsValue error, func()->void reset) => (
              <button onClick={reset}>{JS.string(error)}</button>
            )}
          >
            <Risky />
          </ErrorBoundary>
        </main>
      );
    }
  `);
  assert.match(output, /dynamicErrorBoundary\(region\(/);
  assert.match(output, /JsValue error = JS\.assume\(_boundaryError\d+\)/);
  assert.match(output, /func\(\)->void reset = _boundaryReset\d+/);
  assert.match(output, /componentNodes\(Risky/);
  assert.match(
    output,
    /onDelegatedClickVoid\([^,]+, \(\) => \{ reset\(\); \}\)/,
  );
});

test("rejects an invalid ErrorBoundary reset callback type", () => {
  assert.throws(
    () =>
      compileLilx(`
        DomNode view() {
          return <ErrorBoundary fallback={(JsValue error, int reset) => <p />}>ok</ErrorBoundary>;
        }
      `),
    /ErrorBoundary reset parameter must have type func\(\)->void/,
  );
});

test("lowers Suspense children and component fallback as owned branches", () => {
  const output = compileLilx(`
    DomNode view() {
      return (
        <main>
          <Suspense fallback={<Loading />}>
            <Result value={resource.read()} />
          </Suspense>
        </main>
      );
    }
  `);
  assert.match(output, /dynamicSuspense\(region\(/);
  assert.match(output, /componentNodes\(Result/);
  assert.match(output, /componentNodes\(Loading/);
});

test("avoids event wrappers when delegated handlers ignore the event", () => {
  const output = compileLilx(`
    DomNode view() {
      return <button onClick={() => count.write(count.read() + 1)}>Go</button>;
    }
  `);
  assert.match(output, /onDelegatedClickVoid\(/);
  assert.doesNotMatch(output, /onDelegatedEventVoid\([^,]+, "click"/);
});

test("keeps event-aware delegated handlers when event methods are used", () => {
  const output = compileLilx(`
    DomNode view() {
      return <button onClick={(e) => e.preventDefault()}>Go</button>;
    }
  `);
  assert.match(output, /onDelegatedEvent\(/);
  assert.match(output, /event\.preventDefault\(\)/);
});
