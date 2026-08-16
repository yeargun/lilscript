import { animate, animateMini, scroll, stagger } from "motion";

const keep = [animate, animateMini, scroll, stagger]
  .map((fn) => (typeof fn === "function" ? fn.name || "fn" : "x"))
  .join("|");

const delay = stagger(0.05, { startDelay: 0 });
let schedule = 0;
for (let i = 0; i < 12; i += 1) {
  schedule += delay(i, 12);
}

if (typeof document !== "undefined") {
  const el = document.createElement("div");
  document.body.append(el);
  const anim = animate(el, { x: 100 }, { duration: 0.2 });
  const mini = animateMini(el, { transform: "translateX(100px)" }, { duration: 0.2 });
  scroll(anim, { offset: ["start start", "end end"] });
  scroll(mini, { offset: ["start start", "end end"] });
}

console.log(`animate-scroll:${keep}:${schedule.toFixed(4)}`);
