import { JSDOM } from "jsdom";
import { pathToFileURL } from "node:url";

const dom = new JSDOM(`<!doctype html><html><body></body></html>`);
globalThis.window = dom.window;
globalThis.document = dom.window.document;
for (const key of ["navigator", "location", "HTMLElement", "Node", "Element", "Document"])
  if (!(key in globalThis)) globalThis[key] = dom.window[key];

const mod = await import(
  pathToFileURL("/Users/yeargun/lilscript/benchmarks/popular/build/jquery-layers/manipulation/lilscript.raw.js").href
);
const $ = mod.jQuery ?? globalThis.jQuery ?? dom.window.jQuery;
console.log("jQuery?", typeof $);

const root = document.createElement("div");
root.innerHTML = `<button id="go" type="button">Go</button>`;
document.body.appendChild(root);

const btn = $("#go", root);
console.log("btn.length", btn.length);
const seen = [];
btn.on("click.layer", function (event) {
  seen.push(["click", this.id, event.type]);
});
const events = $._data ? $._data(btn[0], "events") : undefined;
console.log("registered events:", events && Object.keys(events));
btn.trigger("click");
console.log("seen after trigger:", JSON.stringify(seen));
