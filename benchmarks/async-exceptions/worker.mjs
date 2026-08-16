import { pathToFileURL } from "node:url";

const [mode, artifact, nonce] = process.argv.slice(2);
if (global.gc) global.gc();
const before = process.memoryUsage().heapUsed;
let output = "";
const original = console.log;
console.log = (value) => {
  output += `${value}\n`;
};
const start = process.hrtime.bigint();
await import(`${pathToFileURL(artifact).href}?sample=${nonce}`);
const deadline = Date.now() + 5000;
while ((output.match(/\n/g)?.length ?? 0) < 5 && Date.now() < deadline) {
  await new Promise((resolve) => setImmediate(resolve));
}
if ((output.match(/\n/g)?.length ?? 0) < 5) {
  throw new Error(`async artifact did not finish: ${artifact}`);
}
const elapsed = Number(process.hrtime.bigint() - start) / 1e6;
console.log = original;
if (global.gc) global.gc();
const bytes = Math.max(0, process.memoryUsage().heapUsed - before);
process.stdout.write(JSON.stringify({
  output: output.trimEnd(),
  milliseconds: elapsed,
  bytes,
  mode,
}));
