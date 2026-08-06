import { pathToFileURL } from "node:url";

const [warmupText, sampleText, ...artifacts] = process.argv.slice(2);
const warmups = Number(warmupText);
const samples = Number(sampleText);
const artifactUrls = artifacts.map((artifact) => pathToFileURL(artifact).href);
const timings = artifacts.map(() => []);

console.log = () => {};
for (let iteration = 0; iteration < warmups + samples; iteration += 1) {
  for (let offset = 0; offset < artifactUrls.length; offset += 1) {
    const artifactIndex = (iteration + offset) % artifactUrls.length;
    const start = performance.now();
    await import(`${artifactUrls[artifactIndex]}?benchmark-iteration=${iteration}`);
    const elapsed = performance.now() - start;
    if (iteration >= warmups) timings[artifactIndex].push(elapsed);
  }
}

process.stdout.write(JSON.stringify(timings));
