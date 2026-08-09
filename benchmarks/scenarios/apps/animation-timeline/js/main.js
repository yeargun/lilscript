import { cubicBezier, steps } from "@motionone/easing";
import clamp from "clamp";
import lerp from "lerp";

const ease = cubicBezier(0.22, 0.61, 0.36, 1);
const snap = steps(8, "end");
const frames = [];
let digest = 0;
for (let cycle = 0; cycle < 600; cycle += 1) {
  for (let frame = 0; frame <= 120; frame += 1) {
    const progress = clamp(frame / 120, 0, 1);
    const _position = lerp(-24, 320, ease(progress));
    const _opacity = lerp(0.15, 1, snap(progress));
    frames.push({ _position, _opacity });
    digest = (digest + Math.round(_position * 10) + Math.round(_opacity * 100)) % 2147483647;
  }
}
const last = frames[frames.length - 1];
console.log(`animation-timeline:${frames.length}:${digest}:${Math.round(last._position * 100000)}:${Math.round(last._opacity * 100000)}`);
