import { JSDOM } from "jsdom";
import { runJqueryContract } from "../contract.js";

const dom = new JSDOM("<!doctype html><html><body></body></html>");
globalThis.window = dom.window;
globalThis.document = dom.window.document;

const api = await import("./api.js");
runJqueryContract(api.jQuery);
