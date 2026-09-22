import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
const api = await import(pathToFileURL(process.argv[2]).href);
assert.deepEqual(Object.keys(api), ["run"]);
assert.deepEqual([api.run(2,3),api.run(0,0),api.run(-3,4)], [38,10,-15]);
console.log("fixed-resource-family-order-ok");
