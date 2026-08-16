import { createRequire } from "module";
import { JSDOM } from "jsdom";
import { runJqueryContract } from "../contract.js";

const require = createRequire(import.meta.url);
const dom = new JSDOM("<!doctype html><html><body></body></html>");
const $ = require("jquery")(dom.window);
runJqueryContract($);
