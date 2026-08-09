import clamp from "clamp";
import { incirclefast, orient2dfast } from "robust-predicates";

const hits = [];
let digest = 0;
for (let batch = 0; batch < 850; batch += 1) {
  for (let index = 0; index < 48; index += 1) {
    const x = ((index * 37 + batch * 11) % 101) / 100;
    const y = ((index * 19 + batch * 7) % 103) / 102;
    const side = orient2dfast(0, 0, 1, 0, x, y);
    const circle = incirclefast(0, 0, 1, 0, 0, 1, x, y);
    const _confidence = clamp(Math.abs(circle) * 4 + Math.abs(side), 0, 1);
    const _inside = side <= 0 && circle >= 0;
    hits.push({ _confidence, _inside });
    digest = (digest + Math.round(_confidence * 1000) + (_inside ? 97 : 13)) % 2147483647;
  }
}
const last = hits[hits.length - 1];
console.log(`geometry-hit-test:${hits.length}:${digest}:${Math.round(last._confidence * 1000000)}:${last._inside}`);
