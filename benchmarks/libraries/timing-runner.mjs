import { pathToFileURL } from "node:url";

const [warmupText, sampleText, ...artifacts] = process.argv.slice(2);
const warmups = Number(warmupText);
const samples = Number(sampleText);
const urls = artifacts.map((artifact) => pathToFileURL(artifact).href);
const timings = artifacts.map(() => []);

console.log = () => {};
for (let iteration = 0; iteration < warmups + samples; iteration += 1) {
  for (let offset = 0; offset < urls.length; offset += 1) {
    const index = (iteration + offset) % urls.length;
    const start = performance.now();
    await import(`${urls[index]}?library-benchmark=${iteration}`);
    if (iteration >= warmups) timings[index].push(performance.now() - start);
  }
}

process.stdout.write(JSON.stringify(timings));
