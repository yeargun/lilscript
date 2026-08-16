import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";

const [mode, artifact, nonce] = process.argv.slice(2);
const output = [];
console.log = (value) => output.push(String(value));

if (mode === "memory" && typeof global.gc !== "function") {
  throw new Error("memory mode requires --expose-gc");
}

if (mode === "memory") global.gc();
const before = process.memoryUsage();
const started = performance.now();
await import(`${pathToFileURL(artifact).href}?compact-intrinsics=${nonce}`);
const milliseconds = performance.now() - started;
if (mode === "memory") global.gc();
const after = process.memoryUsage();

process.stdout.write(
  JSON.stringify({
    output: output.join("\n"),
    milliseconds,
    bytes:
      after.heapUsed - before.heapUsed +
      after.arrayBuffers - before.arrayBuffers,
  }),
);
