import { performance } from "node:perf_hooks";
import { artifactUrl, checksum, emitResult, nodeEnvironment } from "./common.mjs";

const rendererUrl = artifactUrl();
const renderer = await import(rendererUrl);
const vue = process.env.VUE_LAB_VARIANT === "candidate"
  ? await import(new URL("./vue.runtime.js", rendererUrl))
  : await import("vue");

function createApplication(rowCount, cycle) {
  const Row = {
    props: ["row"],
    render() {
      const row = this.row;
      return vue.h("li", {
        class: { active: row.active, odd: row.id % 2 === 1 },
        "data-id": row.id,
      }, [
        vue.h("strong", null, row.label),
        vue.h("span", null, String(row.value)),
      ]);
    },
  };
  const rows = Array.from({ length: rowCount }, (_, id) => ({
    id,
    label: `row-${id}`,
    value: cycle * 101 + id * id,
    active: (cycle + id) % 3 === 0,
  }));
  return vue.createSSRApp({
    render() {
      return vue.h("main", { id: "app", "data-cycle": cycle }, [
        vue.h("h1", null, `Cycle ${cycle}`),
        vue.h("ul", null, rows.map((row) => vue.h(Row, { key: row.id, row }))),
      ]);
    },
  });
}

await renderer.renderToString(createApplication(8, 0), {});
const cycles = 48;
const rowCount = 48;
const outputs = [];
const started = performance.now();
for (let cycle = 0; cycle < cycles; cycle += 1) {
  const context = {};
  const html = await renderer.renderToString(createApplication(rowCount, cycle), context);
  outputs.push(html, JSON.stringify(context));
}
const durationMs = performance.now() - started;

emitResult({
  durationMs,
  operations: cycles * (rowCount * 3 + 3),
  checksum: checksum(outputs.join("\u0000")),
  environment: nodeEnvironment(),
});
